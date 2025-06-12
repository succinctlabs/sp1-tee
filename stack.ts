import * as cdk from "aws-cdk-lib";
import { Construct } from "constructs";
import { readFileSync } from "fs";

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

        let userData = readFileSync("./scripts/user-data.sh", "utf-8");
        userData = this.base64_encode(userData);

        const launchTemplate = new cdk.aws_ec2.LaunchTemplate(
            this,
            "SP1_TEE_LaunchTemplate",
            {
                instanceType: new cdk.aws_ec2.InstanceType("m5a.4xlarge"),
                machineImage: cdk.aws_ec2.MachineImage.latestAmazonLinux2023(),
                securityGroup: enclaveSg,
                userData: cdk.aws_ec2.UserData.custom(userData),
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
                        subnetType: cdk.aws_ec2.SubnetType.PRIVATE_WITH_EGRESS,
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
                    subnetType: cdk.aws_ec2.SubnetType.PRIVATE_WITH_EGRESS,
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

    base64_encode(str: string): string {
        // create a buffer
        const buff = Buffer.from(str, "utf-8");

        // decode buffer as Base64
        const base64 = buff.toString("base64");

        return base64;
    }
}
