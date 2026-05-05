import * as cdk from "aws-cdk-lib";
import * as cloudwatch from "aws-cdk-lib/aws-cloudwatch";
import * as actions from "aws-cdk-lib/aws-cloudwatch-actions";
import * as s3 from "aws-cdk-lib/aws-s3";
import { Construct } from "constructs";
import { Environment } from "./base";

export interface Sp1TeeVersionedProps extends cdk.StackProps {
    environment: Environment;
    releaseTag: string;
    commit: string;
    vpc: cdk.aws_ec2.Vpc;
    enclaveSg: cdk.aws_ec2.SecurityGroup;
    loadBalancer: cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer;
    httpsListener: cdk.aws_elasticloadbalancingv2.ApplicationListener;
    role: cdk.aws_iam.Role;
    secret: cdk.aws_secretsmanager.ISecret;
    alertsTopic: cdk.aws_sns.Topic;
    /**
     * Private snapshots bucket (created in `Sp1TeeBaseStack`). The
     * `sp1-tee-snapshot-generate` binary running on this host writes the
     * durable `/signers` snapshot here whenever the operator scales the
     * versioned ASG up for regeneration. Write+read grant: write to publish,
     * read for the dry-run path.
     */
    snapshotsBucket: s3.IBucket;
}

// All supported chains
const CHAIN_IDS = [11155111];

export class Sp1TeeVersionedStack extends cdk.Stack {
    constructor(scope: Construct, id: string, props: Sp1TeeVersionedProps) {
        super(scope, id, props);

        const version = parseInt(props.releaseTag.substring(1));

        // Grant the (shared) base role read+write on the snapshots
        // bucket. The role already carries `AmazonS3FullAccess` from the
        // legacy attestations workflow, so functionally this grant is
        // redundant in steady state. It is kept explicit to document the
        // generator's required permissions in IaC; if the base role is
        // ever tightened away from S3FullAccess, the legacy
        // raw-attestations grants would also need to be made explicit at
        // the same time (out of scope for this PR).
        props.snapshotsBucket.grantReadWrite(props.role);

        const userData = this.buildUserData(
            props.environment,
            props.releaseTag,
            props.commit,
            props.secret.secretArn,
            props.snapshotsBucket.bucketName,
        );

        const launchTemplate = new cdk.aws_ec2.LaunchTemplate(
            this,
            `SP1_TEE_LaunchTemplate_${props.releaseTag}`,
            {
                instanceType: new cdk.aws_ec2.InstanceType("m5a.4xlarge"),
                machineImage: cdk.aws_ec2.MachineImage.latestAmazonLinux2023(),
                securityGroup: props.enclaveSg,
                userData,
                nitroEnclaveEnabled: true,
                role: props.role,
                blockDevices: [
                    {
                        deviceName: "/dev/xvda",
                        volume: cdk.aws_ec2.BlockDeviceVolume.ebs(300, {
                            volumeType: cdk.aws_ec2.EbsDeviceVolumeType.GP3,
                            deleteOnTermination: true,
                        }),
                    },
                ],
            },
        );

        const asg = new cdk.aws_autoscaling.AutoScalingGroup(
            this,
            `SP1_TEE_AutoScalingGroup_${props.releaseTag}`,
            {
                // The `/signers` path is now served by Sp1TeeStubStack (Phase
                // 2 cutover). The versioned enclave fleet stays defined so it
                // can be scaled up briefly when a new attestation needs to be
                // generated (SP1 upgrade, key rotation), but idles at zero by
                // default. Scale up: `aws autoscaling set-desired-capacity
                // --desired-capacity 1 --auto-scaling-group-name ...`.
                minCapacity: 0,
                desiredCapacity: 0,
                maxCapacity: 5,
                launchTemplate,
                vpc: props.vpc,
                vpcSubnets: {
                    subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                },
                // The versioned fleet idles at desired=0 by default (Phase 2
                // cutover — `/signers` is served by Sp1TeeStubStack). Allow
                // rolling updates to drop to 0 in-service so launch-template
                // updates don't conflict with the idle baseline. When the
                // fleet is scaled back up for attestation generation,
                // updates will still proceed one instance at a time with the
                // 30-minute cold-start pause window.
                updatePolicy: cdk.aws_autoscaling.UpdatePolicy.rollingUpdate({
                    minInstancesInService: 0,
                    maxBatchSize: 1,
                    pauseTime: cdk.Duration.minutes(30),
                }),
            },
        );

        const targetGroup =
            new cdk.aws_elasticloadbalancingv2.ApplicationTargetGroup(
                this,
                `SP1_TEETargetGroup_${props.releaseTag}`,
                {
                    targets: [asg],
                    protocol:
                        cdk.aws_elasticloadbalancingv2.ApplicationProtocol.HTTP,
                    port: 8080,
                    vpc: props.vpc,
                    healthCheck: {
                        path: "/health",
                        protocol: cdk.aws_elasticloadbalancingv2.Protocol.HTTP,
                        port: "8080",
                    },
                },
            );

        new cdk.aws_elasticloadbalancingv2.ApplicationListenerRule(
            this,
            `SP1_TEE_Rule_${props.releaseTag}`,
            {
                listener: props.httpsListener,
                targetGroups: [targetGroup],
                conditions: [
                    cdk.aws_elasticloadbalancingv2.ListenerCondition.httpHeader(
                        "X-SP1-TEE-Version",
                        [version.toString()],
                    ),
                ],
                priority: 99 + version,
            },
        );

        this.createHealthAlarms(
            props.releaseTag,
            targetGroup,
            props.alertsTopic,
        );
    }

    buildUserData(
        environment: Environment,
        releaseTag: string,
        commit: string,
        secretArn: string,
        snapshotsBucket: string,
    ): cdk.aws_ec2.UserData {
        const userData = cdk.aws_ec2.UserData.forLinux();

        userData.addCommands("dnf install git aws-cli jq -y");
        userData.addCommands("cd /home/ec2-user");

        // Retrieve secrets and add them to .env file
        userData.addCommands(
            `SECRET_JSON=$(aws secretsmanager get-secret-value --secret-id ${secretArn} --region ${this.region} --query SecretString --output text)`,
            "SEAL_URL=$(echo $SECRET_JSON | jq -r .seal_url)",
            "SEAL_BEARER_TOKEN=$(echo $SECRET_JSON | jq -r .seal_bearer_token)",
            "PRIVATE_KEY=$(echo $SECRET_JSON | jq -r .private_key)",
            'echo "SEAL_BEARER_TOKEN=$SEAL_BEARER_TOKEN" >> .env',
            'echo "SEAL_URL=$SEAL_URL" >> .env',
            'echo "PRIVATE_KEY=$PRIVATE_KEY" >> .env',
            `echo "ENCLAVE_TAG=${releaseTag}-${commit}" >> .env`,
            'echo "DISABLE_ALERTS=1" >> .env', // TODO: Remove
        );

        // Add RPC URLs for all supported chains
        CHAIN_IDS.forEach((chainId) => {
            userData.addCommands(
                `RPC_URL_${chainId}=$(echo $SECRET_JSON | jq -r .rpc_url_${chainId})`,
            );
            userData.addCommands(
                `echo "RPC_URL_${chainId}=$RPC_URL_${chainId}" >> .env`,
            );
        });

        // Persist the snapshots bucket name in a stable on-disk location.
        // Operators running the manual `sp1-tee-snapshot-generate` step
        // pick it up from here (see runbook in PR description).
        userData.addCommands(
            "mkdir -p /etc/sp1-tee",
            `echo '${snapshotsBucket}' > /etc/sp1-tee/snapshots-bucket`,
            "chmod 0644 /etc/sp1-tee/snapshots-bucket",
        );

        // Clone the repo
        userData.addCommands(
            "cd /home/ec2-user",
            "git clone https://github.com/succinctlabs/sp1-tee.git",
            "cd sp1-tee",
            `git checkout ${releaseTag}`,
            "chown -R ec2-user:ec2-user .",

            "sudo -u ec2-user ./scripts/install-host.sh" +
                (environment == Environment.Prod ? " --production" : ""),
        );

        return userData;
    }

    createHealthAlarms(
        releaseTag: string,
        targetGroup: cdk.aws_elasticloadbalancingv2.ApplicationTargetGroup,
        alertsTopic: cdk.aws_sns.Topic,
    ) {
        // Alarm for unhealthy targets
        let unhealthyTargetsAlarm = new cloudwatch.Alarm(
            this,
            `SP1_TEE_UnhealthyTargets_Alarm_${releaseTag}`,
            {
                metric: targetGroup.metrics.unhealthyHostCount({
                    period: cdk.Duration.seconds(30),
                    statistic: "Average",
                }),
                threshold: 1,
                comparisonOperator:
                    cloudwatch.ComparisonOperator
                        .GREATER_THAN_OR_EQUAL_TO_THRESHOLD,
                evaluationPeriods: 2,
                treatMissingData: cloudwatch.TreatMissingData.NOT_BREACHING,
                alarmDescription: `SP1 TEE Version ${releaseTag} has unhealthy targets`,
                alarmName: `SP1-TEE-UnhealthyTargets-${releaseTag}`,
            },
        );

        const snsAction = new actions.SnsAction(alertsTopic);

        unhealthyTargetsAlarm.addAlarmAction(snsAction);
        unhealthyTargetsAlarm.addOkAction(snsAction);
    }
}
