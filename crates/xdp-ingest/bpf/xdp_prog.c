// xdp_prog.c - eBPF XDP program for kernel-bypass ingestion
// Filters inbound edge telemetry frames on dedicated UDP port

#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/udp.h>

#define EDGE_TELEMETRY_UDP_PORT 47821

struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 256 * 1024);
} telemetry_ringbuf SEC(".maps");

struct telemetry_meta {
    __u32 src_ip;
    __u32 dst_ip;
    __u16 src_port;
    __u16 dst_port;
    __u16 payload_len;
    __u64 timestamp_ns;
};

SEC("xdp")
int xdp_filter_telemetry(struct xdp_md *ctx) {
    void *data = (void *)(long)ctx->data;
    void *data_end = (void *)(long)ctx->data_end;
    
    struct ethhdr *eth = data;
    if ((void *)(eth + 1) > data_end)
        return XDP_PASS;
    
    if (eth->h_proto != bpf_htons(ETH_P_IP))
        return XDP_PASS;
    
    struct iphdr *iph = data + sizeof(*eth);
    if ((void *)(iph + 1) > data_end)
        return XDP_PASS;
    
    if (iph->protocol != IPPROTO_UDP)
        return XDP_PASS;
    
    struct udphdr *udph = data + sizeof(*eth) + sizeof(*iph);
    if ((void *)(udph + 1) > data_end)
        return XDP_PASS;
    
    __u16 dst_port = bpf_ntohs(udph->dest);
    if (dst_port != EDGE_TELEMETRY_UDP_PORT)
        return XDP_PASS;
    
    // Packet matches our filter - send to ring buffer for user-space consumption
    struct telemetry_meta *meta;
    meta = bpf_ringbuf_reserve(&telemetry_ringbuf, sizeof(*meta), 0);
    if (meta) {
        meta->src_ip = iph->saddr;
        meta->dst_ip = iph->daddr;
        meta->src_port = bpf_ntohs(udph->source);
        meta->dst_port = dst_port;
        meta->payload_len = bpf_ntohs(udph->len) - sizeof(*udph);
        meta->timestamp_ns = bpf_ktime_get_ns();
        bpf_ringbuf_submit(meta, 0);
    }
    
    // Return XDP_TX to send back through driver (zero-copy path)
    // Or XDP_PASS if we want kernel to handle it
    return XDP_PASS;
}

char LICENSE[] SEC("license") = "GPL";
