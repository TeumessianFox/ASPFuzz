# on-chip bootloader

The on-chip bootloader binaries for the different Zen generations have to be gathered, by running attacks from the papers listed below.
After successfully gaining code execution on the ASP, the on-chip bootloader can be dumped.

[Insecure Until Proven Updated: Analyzing AMD SEV's Remote Attestation](https://dl.acm.org/doi/10.1145/3319535.3354216) Buhren et al. 2019 \
[One Glitch to Rule Them All: Fault Injection Attacks Against AMD's Secure Encrypted Virtualization](https://dl.acm.org/doi/10.1145/3460120.3484779) Buhren et al. 2021\
[EM-Fault It Yourself: Building a Replicable EMFI Setup for Desktop and Server Hardware](https://arxiv.org/abs/2209.09835) Kühnapfel et al. 2022

# UEFI images

UEFI images can be gathered from the vendors websites.
Often these come as .CAP files.
Too generate .ROM files the first x bytes have to be removed to exactely reach 16 MiB.
