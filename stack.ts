import * as cdk from "aws-cdk-lib";
import * as acm from "aws-cdk-lib/aws-certificatemanager";
import * as route53 from "aws-cdk-lib/aws-route53";
import * as route53Targets from "aws-cdk-lib/aws-route53-targets";
import { Construct } from "constructs";

const CHAIN_IDS = [11155111];

export class Sp1TeeStack extends cdk.Stack {
    constructor(scope: Construct, id: string, props?: cdk.StackProps) {
        super(scope, id, props);

        const certificateArn = new cdk.CfnParameter(this, "CertificateArn", {
            type: "String",
            description: "ARN of the SSL certificate for HTTPS listener",
        });

        const vpc = new cdk.aws_ec2.Vpc(this, "SP1_TEE_VPC", {
            natGateways: 1,
            enableDnsSupport: true,
            enableDnsHostnames: true,
            subnetConfiguration: [
                {
                    name: "public",
                    subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                },
                {
                    name: "private",
                    subnetType: cdk.aws_ec2.SubnetType.PRIVATE_WITH_EGRESS,
                },
            ],
        });

        // Instance Role and SSM Managed Policy
        const role = new cdk.aws_iam.Role(this, "SP1_TEE_InstanceSSM", {
            assumedBy: new cdk.aws_iam.ServicePrincipal("ec2.amazonaws.com"),
        });

        // Add S3 full access policy
        role.addManagedPolicy(
            cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
                "AmazonS3FullAccess",
            ),
        );

        const enclaveSg = new cdk.aws_ec2.SecurityGroup(
            this,
            "SP1_TEE_SecurityGroup",
            {
                vpc,
                allowAllOutbound: true,
                description: "Private SG for SP1 TEE enclaves",
            },
        );

        enclaveSg.addIngressRule(
            cdk.aws_ec2.Peer.anyIpv4(),
            cdk.aws_ec2.Port.tcp(22),
            "Allow SSH access",
        );

        const secret = cdk.aws_secretsmanager.Secret.fromSecretNameV2(
            this,
            "SP1_TEE_Secret",
            "sp1_tee",
        );

        secret.grantRead(role);

        const loadBalancer =
            new cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer(
                this,
                "SP1_TEE_ApplicationLoadBalancer",
                {
                    vpc,
                    vpcSubnets: {
                        subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                    },
                    internetFacing: true,
                },
            );

        const hostedZone = route53.HostedZone.fromHostedZoneAttributes(
            this,
            "SP1_TEE_HostedZone",
            {
                hostedZoneId: "Z07931692VMB8INXEKFYF", // TODO: Update
                zoneName: "succinct.tools",
            },
        );

        // Create A record (alias) pointing domain to load balancer
        new route53.ARecord(this, "SP1_TEE_ARecord", {
            zone: hostedZone,
            recordName: "tee",
            target: route53.RecordTarget.fromAlias(
                new route53Targets.LoadBalancerTarget(loadBalancer),
            ),
            ttl: undefined, // TTL is automatically set for alias records
        });

        const certificate = acm.Certificate.fromCertificateArn(
            this,
            "SP1_TEE_Certificate",
            certificateArn.valueAsString,
        );

        const httpsListener = loadBalancer.addListener(
            "SP1_TEE_ApplicationLoadBalancer_HTTPSListener",
            {
                port: 443,
                defaultAction:
                    cdk.aws_elasticloadbalancingv2.ListenerAction.fixedResponse(
                        400,
                        {
                            contentType: "text/plain",
                            messageBody: "Missing version",
                        },
                    ),
                protocol:
                    cdk.aws_elasticloadbalancingv2.ApplicationProtocol.HTTPS,
                certificates: [certificate],
            },
        );

        this.createVersionedInfrastructure(
            "1",
            secret.secretArn,
            vpc,
            enclaveSg,
            role,
            httpsListener,
        );
    }

    createVersionedInfrastructure(
        version: string,
        secretArn: string,
        vpc: cdk.aws_ec2.Vpc,
        enclaveSg: cdk.aws_ec2.SecurityGroup,
        role: cdk.aws_iam.Role,
        httpsListener: cdk.aws_elasticloadbalancingv2.ApplicationListener,
    ) {
        const userData = this.buildUserData(version, secretArn);

        const launchTemplate = new cdk.aws_ec2.LaunchTemplate(
            this,
            `SP1_TEE_LaunchTemplate_V${version}`,
            {
                instanceType: new cdk.aws_ec2.InstanceType("m5a.4xlarge"),
                machineImage: cdk.aws_ec2.MachineImage.latestAmazonLinux2023(),
                securityGroup: enclaveSg,
                userData,
                nitroEnclaveEnabled: true,
                role,
                blockDevices: [
                    {
                        deviceName: "/dev/xvda",
                        volume: cdk.aws_ec2.BlockDeviceVolume.ebs(50, {
                            volumeType: cdk.aws_ec2.EbsDeviceVolumeType.GP3,
                            deleteOnTermination: true,
                        }),
                    },
                ],
            },
        );

        const asg = new cdk.aws_autoscaling.AutoScalingGroup(
            this,
            `SP1_TEE_AutoScalingGroup_V${version}`,
            {
                minCapacity: 2,
                maxCapacity: 5,
                launchTemplate,
                vpc,
                vpcSubnets: {
                    subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                },
                updatePolicy: cdk.aws_autoscaling.UpdatePolicy.rollingUpdate(),
            },
        );

        const targetGroup =
            new cdk.aws_elasticloadbalancingv2.ApplicationTargetGroup(
                this,
                `SP1_TEE_TargetGroup_V${version}`,
                {
                    targets: [asg],
                    protocol:
                        cdk.aws_elasticloadbalancingv2.ApplicationProtocol.HTTP,
                    port: 8080,
                    vpc,
                    healthCheck: {
                        path: "/health",
                        protocol: cdk.aws_elasticloadbalancingv2.Protocol.HTTP,
                        port: "8080",
                    },
                },
            );

        httpsListener.addTargetGroups(`SP1_TEE_V${version}_Rule`, {
            targetGroups: [targetGroup],
            conditions: [
                cdk.aws_elasticloadbalancingv2.ListenerCondition.httpHeader(
                    "X-SP1-TEE-Version",
                    [version],
                ),
            ],
            priority: 100,
        });
    }

    buildUserData(version: string, secretArn: string): cdk.aws_ec2.UserData {
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
            `echo "ENCLAVE_VERSION=${version}" >> .env`,
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

        // Clone the repo
        userData.addCommands(
            "cd /home/ec2-user",
            "git clone https://github.com/succinctlabs/sp1-tee.git",
            "cd sp1-tee",
            "git checkout aurelien/automate-deployments", // TODO: Remove
            "mv Dockerfile.enclave Dockerfile",
            "chown -R ec2-user:ec2-user .",

            "sudo -u ec2-user ./scripts/install-host.sh", // TODO: Add --production
        );

        return userData;
    }
}
