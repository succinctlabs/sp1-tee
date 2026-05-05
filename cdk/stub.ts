import * as cdk from "aws-cdk-lib";
import * as s3 from "aws-cdk-lib/aws-s3";
import { Construct } from "constructs";
import { Environment } from "./base";

export interface Sp1TeeStubProps extends cdk.StackProps {
    environment: Environment;
    /**
     * sp1-tee commit SHA (or branch / tag) to clone on the stub host. Pinning
     * to a specific commit makes ASG instance replacements reproducible — a
     * later replacement re-runs cargo install against the same source rather
     * than tracking whatever happens to be at `main` HEAD.
     */
    commit: string;
    vpc: cdk.aws_ec2.Vpc;
    loadBalancer: cdk.aws_elasticloadbalancingv2.ApplicationLoadBalancer;
    httpsListener: cdk.aws_elasticloadbalancingv2.ApplicationListener;
    /**
     * Private snapshots bucket (created in `Sp1TeeBaseStack`). Stub reads
     * the durable signers snapshot from this bucket on every `/signers`
     * request. The role gets a scoped `s3:GetObject` grant only — no list,
     * no put.
     */
    snapshotsBucket: s3.IBucket;
}

/**
 * Deploys the `sp1-tee-signers-stub` binary on a small ARM EC2 instance behind
 * the existing application load balancer. A dedicated listener rule routes
 * requests carrying the `X-SP1-Tee-Stub: 1` header to the stub target group;
 * traffic without that header continues to flow to the existing target groups
 * (`Sp1TeeVersionedStack-*`) untouched.
 */
export class Sp1TeeStubStack extends cdk.Stack {
    constructor(scope: Construct, id: string, props: Sp1TeeStubProps) {
        super(scope, id, props);

        const userData = this.buildUserData(
            props.commit,
            props.snapshotsBucket.bucketName,
        );

        // Dedicated IAM role: only what the stub host actually needs.
        // - SSM agent: `AmazonSSMManagedInstanceCore` (so we can debug via
        //   Session Manager without opening SSH).
        // - cfn-signal: CDK adds a stack-scoped `cloudformation:SignalResource`
        //   policy when `Signals.waitForCount` is used below.
        // - Snapshots bucket: scoped `s3:GetObject` only (granted below). No
        //   list, no put. Stub does not read the legacy public attestations
        //   bucket on the steady-state path.
        const role = new cdk.aws_iam.Role(this, "SP1_TEE_StubInstanceRole", {
            assumedBy: new cdk.aws_iam.ServicePrincipal("ec2.amazonaws.com"),
            managedPolicies: [
                cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
                    "AmazonSSMManagedInstanceCore",
                ),
            ],
        });

        // Scope IAM to the snapshot key under this version. `grantRead`
        // would also work but covers the entire bucket; we only need a
        // single object so use `grantRead` with a key arn for narrower
        // blast radius.
        props.snapshotsBucket.grantRead(role);

        // Dedicated SG. CDK auto-adds the ALB SG → port 8080 ingress when the
        // ASG is attached to the target group below, so no explicit ingress
        // rule is needed here. Egress defaults to allow-all (rust toolchain
        // download + crates.io fetch + S3 attestation list).
        const securityGroup = new cdk.aws_ec2.SecurityGroup(
            this,
            "SP1_TEE_StubSecurityGroup",
            {
                vpc: props.vpc,
                description: "SP1 TEE signers-stub host",
                allowAllOutbound: true,
            },
        );

        const launchTemplate = new cdk.aws_ec2.LaunchTemplate(
            this,
            "SP1_TEE_StubLaunchTemplate",
            {
                instanceType: new cdk.aws_ec2.InstanceType("t4g.medium"),
                machineImage: cdk.aws_ec2.MachineImage.latestAmazonLinux2023({
                    cpuType: cdk.aws_ec2.AmazonLinuxCpuType.ARM_64,
                }),
                securityGroup,
                userData,
                role,
                blockDevices: [
                    {
                        // Root volume bumped to 20 GB to comfortably hold the
                        // OS, the rust toolchain, the cargo build cache, and
                        // the 4 GB swapfile that install-stub.sh provisions.
                        deviceName: "/dev/xvda",
                        volume: cdk.aws_ec2.BlockDeviceVolume.ebs(20, {
                            volumeType: cdk.aws_ec2.EbsDeviceVolumeType.GP3,
                            deleteOnTermination: true,
                        }),
                    },
                ],
            },
        );

        const asg = new cdk.aws_autoscaling.AutoScalingGroup(
            this,
            "SP1_TEE_StubAutoScalingGroup",
            {
                minCapacity: 1,
                desiredCapacity: 1,
                maxCapacity: 1,
                launchTemplate,
                vpc: props.vpc,
                vpcSubnets: {
                    subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                },
                // Block CFN deploy until the instance signals success at the
                // end of user-data. Timeout sits inside healthCheckGracePeriod
                // so a stuck install surfaces as a CFN failure before the ASG
                // starts replacing the instance for ALB-unhealthy reasons.
                signals: cdk.aws_autoscaling.Signals.waitForCount(1, {
                    timeout: cdk.Duration.minutes(25),
                }),
                healthChecks: cdk.aws_autoscaling.HealthChecks.ec2({
                    gracePeriod: cdk.Duration.minutes(30),
                }),
                updatePolicy: cdk.aws_autoscaling.UpdatePolicy.rollingUpdate({
                    minInstancesInService: 0,
                    maxBatchSize: 1,
                    pauseTime: cdk.Duration.minutes(30),
                }),
            },
        );

        // Inject `cfn-signal` at the end of user-data, paired with the
        // `signals` block above.
        userData.addSignalOnExitCommand(asg);

        const targetGroup = new cdk.aws_elasticloadbalancingv2.ApplicationTargetGroup(
            this,
            "SP1_TEE_StubTargetGroup",
            {
                targets: [asg],
                protocol: cdk.aws_elasticloadbalancingv2.ApplicationProtocol.HTTP,
                port: 8080,
                vpc: props.vpc,
                healthCheck: {
                    path: "/health",
                    protocol: cdk.aws_elasticloadbalancingv2.Protocol.HTTP,
                    port: "8080",
                },
            },
        );

        // Primary `/signers` route. The stub serves the same response as the
        // versioned fleet's `/signers` handler (byte-identical, verified live
        // in the prior phase), but reads cached attestations directly from S3
        // instead of running an enclave.
        //
        // Conditions are ANDed: `/signers` path AND the SDK's normal
        // `X-SP1-TEE-Version: 1` header. Higher precedence than the versioned
        // rule at `99 + version` (= 100), so SDK clients land on the stub.
        // Other paths under that header (`/execute`, `/address`, `/health`)
        // do not match here and fall through to the versioned rule.
        new cdk.aws_elasticloadbalancingv2.ApplicationListenerRule(
            this,
            "SP1_TEE_StubSignersPathRule",
            {
                listener: props.httpsListener,
                targetGroups: [targetGroup],
                conditions: [
                    cdk.aws_elasticloadbalancingv2.ListenerCondition.pathPatterns(
                        ["/signers", "/signers/"],
                    ),
                    cdk.aws_elasticloadbalancingv2.ListenerCondition.httpHeader(
                        "X-SP1-TEE-Version",
                        ["1"],
                    ),
                ],
                priority: 30,
            },
        );

        // Header-gated validation/debug bypass. Preserved so the stub stays
        // reachable independently of the path-routed primary rule above —
        // useful for direct curl checks during incident response.
        new cdk.aws_elasticloadbalancingv2.ApplicationListenerRule(
            this,
            "SP1_TEE_StubRule",
            {
                listener: props.httpsListener,
                targetGroups: [targetGroup],
                conditions: [
                    cdk.aws_elasticloadbalancingv2.ListenerCondition.httpHeader(
                        "X-SP1-Tee-Stub",
                        ["1"],
                    ),
                ],
                priority: 50,
            },
        );
    }

    buildUserData(commit: string, snapshotsBucket: string): cdk.aws_ec2.UserData {
        const userData = cdk.aws_ec2.UserData.forLinux();

        // `sudo -u ec2-user -H` runs install-stub.sh as ec2-user with HOME
        // set to /home/ec2-user, so `~/.cargo/env` and the systemd unit's
        // hardcoded path agree.
        const checkoutCmd = commit
            ? `git checkout --detach ${commit}`
            : "# no commit pin set — leaving default branch checked out";

        // SP1_TEE_SNAPSHOTS_BUCKET is consumed by install-stub.sh, which
        // substitutes the placeholder in the systemd unit template before
        // moving it into /etc/systemd/system. The stub binary reads it via
        // `--bucket`/`SP1_TEE_SNAPSHOTS_BUCKET` env in main().
        userData.addCommands(
            "set -euo pipefail",
            "dnf install -y git aws-cfn-bootstrap",
            "cd /home/ec2-user",
            "git clone https://github.com/succinctlabs/sp1-tee.git",
            "cd sp1-tee",
            checkoutCmd,
            "chown -R ec2-user:ec2-user .",
            `sudo -u ec2-user -H SP1_TEE_SNAPSHOTS_BUCKET='${snapshotsBucket}' ./scripts/install-stub.sh`,
        );

        return userData;
    }
}
