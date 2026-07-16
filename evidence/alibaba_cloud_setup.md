# Alibaba Cloud TEE Deployment Runbook

> **Status: PLANNED — deployment has not been performed.**
> This document is a runbook: the steps to execute and the evidence to
> capture when the deployment happens. Nothing below is a record of a
> completed deployment. When each step is performed, replace the
> `TODO(capture)` markers with real command output, keep raw logs in this
> directory, and update the status line above.

## 1. Provision the Confidential VM

Target specification (choose one instance family and one region at
deployment time):

- **Instance type**: `ecs.g7t.xlarge` (Intel SGX) — or `ecs.c7t` (AMD SEV)
- **Region**: pick per data-residency requirement (e.g. `ap-southeast-1` or `cn-hangzhou`)
- **vCPU / RAM**: 8 cores / 32 GB
- **OS**: Alibaba Cloud Linux 3 (kernel 5.10+, eBPF support)

`TODO(capture)`: real instance ID, region, and the console screenshot after
provisioning.

## 2. TEE Environment Setup

```bash
# Install SGX/SEV drivers
yum install -y sgx-driver sev-guest

# Verify TEE attestation capability
/opt/alibaba/teeservice/bin/attestation_verify

# Check enclave memory
cat /sys/kernel/debug/x86/sgx/enclaves
```

`TODO(capture)`: full output of `attestation_verify` and the CAS
(Confidential Attestation Service) report JSON — including the real
enclave measurement hash.

## 3. Security Group Rules

| Direction | Protocol | Port | Source/Destination | Purpose |
|-----------|----------|------|--------------------|---------|
| Inbound   | UDP      | 47821 | edge node CIDR only | Edge telemetry ingestion |
| Outbound  | TCP      | 443   | dashscope.aliyuncs.com | Qwen Cloud API calls |
| Outbound  | UDP      | 47821 | edge node CIDR only | Responses to edge nodes |

Note: restrict inbound to known edge CIDRs — do **not** use `0.0.0.0/0`
for the telemetry port in production.

`TODO(capture)`: security-group rule listing from the console or
`aliyun ecs DescribeSecurityGroupAttribute`.

## 4. Deploy the Gateway

Prerequisite (code work, tracked in the repo): `tee-gateway` is currently
a library crate — a binary target with real HTTP calls to the Qwen API
(`call_qwen_api` is mocked today) and real SGX/SEV sealing
(`SealedStorage` is simulated today) must exist before this step is
meaningful.

```bash
cargo build --release -p tee-gateway   # once a binary target exists
scp target/release/tee_gateway <instance>:/opt/sovereign-edge/
```

Qwen API configuration to apply on the instance:

```yaml
qwen_api:
  endpoint: https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation
  model: qwen-max
  api_key_location: /var/lib/tee/sealed/qwen_token.enc
  timeout_ms: 30000
  retry_count: 3
```

`TODO(capture)`: `ps aux` showing the running gateway, `ss -tuln` showing
the bound UDP port, and one real request/response pair (token redacted).

## 5. Evidence to Capture for Submission

When the deployment is real, this directory should contain:

1. Instance details (ID, type, region) + console screenshot
2. Raw CAS attestation report JSON with the real enclave hash
3. Security-group listing
4. Gateway process + socket listing from the instance
5. Latency/throughput measurements from an actual edge node, with the
   measurement methodology (tool, duration, frame sizes)
6. Measured Qwen API response times (not vendor-published numbers)
7. Monthly cost readout from the billing console after a representative day

## Cost Estimate (pre-deployment, from published pricing)

| Component | Unit Price | Quantity | Monthly Est. |
|-----------|-----------|----------|--------------|
| ecs.g7t.xlarge | $0.15/hr | 1 | ~$108 |
| Data Transfer | $0.08/GB | 500 GB | ~$40 |
| Qwen API Calls | $0.002/1K tokens | 5M tokens | ~$10 |
| **Total** | | | **~$158/month** |

These are estimates from published pricing, not billed amounts.

## Compliance Targets

1. **Data Residency**: all processing within chosen Alibaba Cloud region
2. **Encryption**: PQC in transit, TEE sealing at rest (once real sealing lands)
3. **Audit Trail**: execution logs exported for compliance verification
4. **Access Control**: RAM roles restricting API access to the TEE gateway
