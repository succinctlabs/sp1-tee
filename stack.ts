import * as cdk from "aws-cdk-lib";
import { Construct } from "constructs";

export class Sp1TeeStack extends cdk.Stack {
    constructor(scope: Construct, id: string, props?: cdk.StackProps) {
        super(scope, id, props);

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

        role.addManagedPolicy(
            cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
                "service-role/AmazonEC2RoleforSSM",
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

        const userData = cdk.aws_ec2.UserData.forLinux();
        userData.addCommands(
            "su ec2-user",
            "sudo dnf install git aws-cli jq -y",

            "cd /home/ec2-user",
            "git clone https://github.com/succinctlabs/sp1-tee.git",
            "cd sp1-tee",
            "git checkout aurelien/automate-deployments", // TODO: Remove

            "export HOME=/home/ec2-user",

            // Retrieve secrets and add them to .env file
            `SECRET_JSON=$(aws secretsmanager get-secret-value --secret-id ${secret.secretArn} --region ${this.region} --query SecretString --output text)`,
            "SEAL_URL=$(echo $SECRET_JSON | jq -r .seal_url)",
            "SEAL_BEARER_TOKEN=$(echo $SECRET_JSON | jq -r .seal_bearer_token)",
            'echo "SEAL_URL=$SEAL_URL" >> .env',
            'echo "SEAL_BEARER_TOKEN=$SEAL_BEARER_TOKEN" >> .env',

            "./scripts/install-host.sh", // TODO: Add --production
        );

        const launchTemplate = new cdk.aws_ec2.LaunchTemplate(
            this,
            "SP1_TEE_LaunchTemplate",
            {
                instanceType: new cdk.aws_ec2.InstanceType("m5a.4xlarge"),
                machineImage: cdk.aws_ec2.MachineImage.latestAmazonLinux2023(),
                securityGroup: enclaveSg,
                userData,
                nitroEnclaveEnabled: true,
                role,
            },
        );

        const loadBalancer =
            new cdk.aws_elasticloadbalancingv2.NetworkLoadBalancer(
                this,
                "SP1_TEE_NetworkLoadBalancer",
                {
                    vpc,
                    vpcSubnets: {
                        subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                    },
                    internetFacing: false,
                },
            );

        const asg = new cdk.aws_autoscaling.AutoScalingGroup(
            this,
            "SP1_TEE_AutoScalingGroup",
            {
                minCapacity: 2,
                maxCapacity: 2,
                launchTemplate,
                vpc,
                vpcSubnets: {
                    subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                },
                updatePolicy: cdk.aws_autoscaling.UpdatePolicy.rollingUpdate(),
            },
        );

        loadBalancer.addListener("SP1_TEE_NetworkLoadBalancer_HTTPSListener", {
            port: 443,
            protocol: cdk.aws_elasticloadbalancingv2.Protocol.TCP,
            defaultTargetGroups: [
                new cdk.aws_elasticloadbalancingv2.NetworkTargetGroup(
                    this,
                    "SP1_TEE_NetworkLoadBalancer_AutoScalingGroupTarget",
                    {
                        targets: [asg],
                        protocol: cdk.aws_elasticloadbalancingv2.Protocol.TCP,
                        port: 443,
                        vpc,
                    },
                ),
            ],
        });
    }
}
