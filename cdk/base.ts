import * as cdk from "aws-cdk-lib";
import * as acm from "aws-cdk-lib/aws-certificatemanager";
import * as route53 from "aws-cdk-lib/aws-route53";
import * as route53Targets from "aws-cdk-lib/aws-route53-targets";
import * as sns from "aws-cdk-lib/aws-sns";
import * as snsSubscriptions from "aws-cdk-lib/aws-sns-subscriptions";
import { Construct } from "constructs";

export interface Sp1TeeBaseProps extends cdk.StackProps {
    environment: Environment;
    certificateArn: string | undefined;
    hostedZoneId: string | undefined;
    zoneName: string;
    pagerDutyWebhookUrl: string | undefined;
}

export enum Environment {
    Staging = "-staging",
    Prod = "",
}

export class Sp1TeeBaseStack extends cdk.Stack {
    public readonly vpc: cdk.aws_ec2.Vpc;
    public readonly loadBalancer: cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer;
    public readonly enclaveSg: cdk.aws_ec2.SecurityGroup;
    public readonly httpsListener: cdk.aws_elasticloadbalancingv2.ApplicationListener;
    public readonly role: cdk.aws_iam.Role;
    public readonly secret: cdk.aws_secretsmanager.ISecret;
    public readonly alertsTopic: cdk.aws_sns.Topic;

    constructor(scope: Construct, id: string, props: Sp1TeeBaseProps) {
        super(scope, id, props);

        if (!props.certificateArn) {
            throw `The CERTIFICATE_ARN env variable is required`;
        }

        if (!props.hostedZoneId) {
            throw `The HOSTED_ZONE_ID env variable is required`;
        }

        this.alertsTopic = new sns.Topic(
            this,
            `SP1_TEE_HealthAlerts${props.environment}`,
            {
                displayName: "SP1 TEE Health Alerts",
                topicName: "sp1-tee-health-alerts",
            },
        );

        if (props.pagerDutyWebhookUrl) {
            this.alertsTopic.addSubscription(
                new snsSubscriptions.UrlSubscription(
                    props.pagerDutyWebhookUrl,
                    {
                        protocol: sns.SubscriptionProtocol.HTTPS,
                        rawMessageDelivery: true,
                    },
                ),
            );
        }

        this.vpc = new cdk.aws_ec2.Vpc(
            this,
            `SP1_TEE_VPC${props.environment}`,
            {
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
            },
        );

        // Instance Role and SSM Managed Policy
        this.role = new cdk.aws_iam.Role(
            this,
            `SP1_TEE_InstanceSSM${props.environment}`,
            {
                assumedBy: new cdk.aws_iam.ServicePrincipal(
                    "ec2.amazonaws.com",
                ),
            },
        );

        // Add S3 full access policy
        this.role.addManagedPolicy(
            cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
                "AmazonS3FullAccess",
            ),
        );

        // Add SSM managed policy for Session Manager
        this.role.addManagedPolicy(
            cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
                "AmazonSSMManagedInstanceCore",
            ),
        );

        this.enclaveSg = new cdk.aws_ec2.SecurityGroup(
            this,
            `SP1_TEE_SecurityGroup${props.environment}`,
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
            `SP1_TEE_Secret${props.environment}`,
            "sp1_tee",
        );

        this.secret.grantRead(this.role);

        this.loadBalancer =
            new cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer(
                this,
                `SP1_TEE_ApplicationLoadBalance${props.environment}`,
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
            `SP1_TEE_HostedZone${props.environment}`,
            {
                hostedZoneId: props.hostedZoneId,
                zoneName: props.zoneName,
            },
        );

        // Create A record (alias) pointing domain to load balancer
        new route53.ARecord(this, `SP1_TEE_ARecord${props.environment}`, {
            zone: hostedZone,
            recordName: "tee2",
            target: route53.RecordTarget.fromAlias(
                new route53Targets.LoadBalancerTarget(this.loadBalancer),
            ),
            ttl: undefined, // TTL is automatically set for alias records
        });

        const certificate = acm.Certificate.fromCertificateArn(
            this,
            `SP1_TEE_Certificate${props.environment}`,
            props.certificateArn,
        );

        this.httpsListener = this.loadBalancer.addListener(
            `SP1_TEE_ApplicationLoadBalancer_HTTPSListener${props.environment}`,
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
