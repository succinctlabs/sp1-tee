#!/usr/bin/env node
import * as cdk from "aws-cdk-lib";
import { Sp1TeeBaseStack, Environment } from "./cdk/base";
import { Sp1TeeVersionedStack } from "./cdk/versioned";

const app = new cdk.App();
const releaseTag = process.env.RELEASE_TAG;
const commit = process.env.SHORT_SHA || "";
const prodReleaseRegEx = /^v[0-9]+$/;

if (!releaseTag) {
    throw "Please provide a valid RELEASE_TAG env variable";
}

const environment = prodReleaseRegEx.test(releaseTag) ? Environment.Prod : Environment.Staging;

let props = undefined;

switch(environment) {
    case Environment.Prod:
        props = {
            env: { account: process.env.CDK_DEFAULT_ACCOUNT, region: process.env.CDK_DEFAULT_REGION },
            environment,
            certificateArn: process.env.CERTIFICATE_ARN,
            hostedZoneId: process.env.HOSTED_ZONE_ID,
            zoneName: "production.succinct.xyz",
            pagerDutyWebhookUrl: process.env.PAGER_DUTY_WEB_HOOK_URL,
        }

        break;
    
    case Environment.Staging:
        props = {
            env:{ account: process.env.CDK_DEFAULT_ACCOUNT, region: "us-west-1" },
            environment,
            certificateArn: process.env.CERTIFICATE_ARN_STAGING,
            hostedZoneId: process.env.HOSTED_ZONE_ID_STAGING,
            zoneName: "succinct.tools",
            pagerDutyWebhookUrl: undefined,
        }

        break;
}

const base = new Sp1TeeBaseStack(app, `Sp1TeeBaseStack${environment}`, props);

new Sp1TeeVersionedStack(app, `Sp1TeeVersionedStack-${releaseTag}`, {
    env: { account: process.env.CDK_DEFAULT_ACCOUNT, region: "us-west-1" },

    environment,
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
