#!/usr/bin/env node
import * as cdk from "aws-cdk-lib";
import { Sp1TeeBaseStack } from "./cdk/base";
import { Sp1TeeVersionedStack } from "./cdk/versioned";

const app = new cdk.App();
const releaseTag = process.env.RELEASE_TAG;
const commit = process.env.SHORT_SHA || "";

if (!releaseTag) {
    throw "Please provide a valid RELEASE_TAG env variable";
}

const base = new Sp1TeeBaseStack(app, "Sp1TeeBaseStack", {
    /* If you don't specify 'env', this stack will be environment-agnostic.
     * Account/Region-dependent features and context lookups will not work,
     * but a single synthesized template can be deployed anywhere. */
    env: { account: process.env.CDK_DEFAULT_ACCOUNT, region: "us-west-1" },
});

new Sp1TeeVersionedStack(app, `Sp1TeeVersionedStack-${releaseTag}`, {
    env: { account: process.env.CDK_DEFAULT_ACCOUNT, region: "us-west-1" },

    releaseTag,
    commit,
    vpc: base.vpc,
    enclaveSg: base.enclaveSg,
    loadBalancer: base.loadBalancer,
    httpsListener: base.httpsListener,
    role: base.role,
    secret: base.secret,
    alertsTopic: base.alertsTopic,
});
