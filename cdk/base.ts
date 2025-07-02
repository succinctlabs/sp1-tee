import * as cdk from "aws-cdk-lib";
import * as acm from "aws-cdk-lib/aws-certificatemanager";
import * as route53 from "aws-cdk-lib/aws-route53";
import * as route53Targets from "aws-cdk-lib/aws-route53-targets";
import * as sns from "aws-cdk-lib/aws-sns";
import * as snsSubscriptions from "aws-cdk-lib/aws-sns-subscriptions";
import { Construct } from "constructs";

export class Sp1TeeBaseStack extends cdk.Stack {
    public readonly vpc: cdk.aws_ec2.Vpc;
    public readonly loadBalancer: cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer;
    public readonly enclaveSg: cdk.aws_ec2.SecurityGroup;
    public readonly httpsListener: cdk.aws_elasticloadbalancingv2.ApplicationListener;
    public readonly role: cdk.aws_iam.Role;
    public readonly secret: cdk.aws_secretsmanager.ISecret;
    public readonly alertsTopic: cdk.aws_sns.Topic;

    constructor(scope: Construct, id: string, props?: cdk.StackProps) {
        super(scope, id, props);

        const certificateArn = new cdk.CfnParameter(this, "CertificateArn", {
            type: "String",
            description: "ARN of the SSL certificate for HTTPS listener",
        });

        /*
        const pagerDutyWebhookUrl = new cdk.CfnParameter(
            this,
            "PagerDutyWebhookUrl",
            {
                type: "String",
                description: "PagerDuty webhook URL",
            },
        );
         */

        this.alertsTopic = new sns.Topic(this, "SP1_TEE_HealthAlerts", {
            displayName: "SP1 TEE Health Alerts",
            topicName: "sp1-tee-health-alerts",
        });

        /*
        if (pagerDutyWebhookUrl.valueAsString) {
            this.alertsTopic.addSubscription(
                new snsSubscriptions.UrlSubscription(
                    pagerDutyWebhookUrl.valueAsString,
                    {
                        protocol: sns.SubscriptionProtocol.HTTPS,
                        rawMessageDelivery: true,
                    },
                ),
            );
        }
         */

        this.vpc = new cdk.aws_ec2.Vpc(this, "SP1_TEE_VPC", {
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
        this.role = new cdk.aws_iam.Role(this, "SP1_TEE_InstanceSSM", {
            assumedBy: new cdk.aws_iam.ServicePrincipal("ec2.amazonaws.com"),
        });

        // Add S3 full access policy
        this.role.addManagedPolicy(
            cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
                "AmazonS3FullAccess",
            ),
        );

        this.enclaveSg = new cdk.aws_ec2.SecurityGroup(
            this,
            "SP1_TEE_SecurityGroup",
            {
                vpc: this.vpc,
                allowAllOutbound: true,
                description: "Private SG for SP1 TEE enclaves",
            },
        );

        this.enclaveSg.addIngressRule(
            cdk.aws_ec2.Peer.anyIpv4(),
            cdk.aws_ec2.Port.tcp(22),
            "Allow SSH access",
        );

        this.secret = cdk.aws_secretsmanager.Secret.fromSecretNameV2(
            this,
            "SP1_TEE_Secret",
            "sp1_tee",
        );

        this.secret.grantRead(this.role);

        this.loadBalancer =
            new cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer(
                this,
                "SP1_TEE_ApplicationLoadBalancer",
                {
                    vpc: this.vpc,
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
                new route53Targets.LoadBalancerTarget(this.loadBalancer),
            ),
            ttl: undefined, // TTL is automatically set for alias records
        });

        const certificate = acm.Certificate.fromCertificateArn(
            this,
            "SP1_TEE_Certificate",
            certificateArn.valueAsString,
        );

        this.httpsListener = this.loadBalancer.addListener(
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
    }
}
