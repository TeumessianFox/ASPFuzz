#!/bin/env bash

ASP_FLASH_IMAGE=""
QEMU_PATH=""
ROM_BL=""
ZEN_GENERATION=""
PORT="1234"

echo "Script to run a flash image in qemu with debugging active"
echo "Commandline inputs:"
echo "1:                        Zen generation (Zen1,Zen+,Zen2,Zen3,ZenTesla)"
echo "2:                        Path to flash image"
echo "Environment variables:"
echo "GDB_PORT:                 (Optional) GDB port"
echo "ASP_ROM_BL:               (Optional) ROM bootloader file"
echo "QEMU_PATH:                (Optional) Path to QEMU"
echo ""

if [ -z $1 ]
then
    echo "ERROR: Use commandline input 1 to set the zen gerations (Zen1,Zen+,Zen2,Zen3,ZenTesla)"
    exit 1
fi

case $1 in

  "Zen1")
    ROM_BL="../fuzzer/amd_sp/bins/on-chip-bl-Ryzen-Zen1-Desktop"
    ZEN_GENERATION="zen"
    ;;

  "Zen+")
    ROM_BL="../fuzzer/amd_sp/bins/on-chip-bl-Ryzen-Zen+-Desktop"
    ZEN_GENERATION="zen+"
    ;;

  "Zen2")
    ROM_BL="../fuzzer/amd_sp/bins/on-chip-bl-Ryzen-Zen2-Desktop"
    ZEN_GENERATION="zen2"
    ;;

  "Zen3")
    ROM_BL="../fuzzer/amd_sp/bins/on-chip-bl-Ryzen-Zen3-Desktop"
    ZEN_GENERATION="zen3"
    ;;

  "ZenTesla")
    ROM_BL="bins/on-chip-bl-Ryzen-ZenTesla"
    ZEN_GENERATION="zentesla"
    ;;

  *)
    echo "ERROR: Use commandline input 1 to set the Zen gerations (Zen1,Zen+,Zen2,Zen3,ZenTesla)"
    exit 1
    ;;
esac


if [ -z $2 ]
then
    echo "ERROR: Use commandline input 2 to set flash image path"
    exit 1
else
    if [ -e $2 ]
    then
        ASP_FLASH_IMAGE=$2
    else
        echo "ERROR: $2 is not a file"
        exit 1
    fi
fi

if [ ! -z $GDB_PORT ]
then
    PORT=$GDB_PORT
fi

if [ ! -z $ASP_ROM_BL ]
then
    ROM_BL=$ASP_ROM_BL
fi

if [ -z $QEMU_PATH ]
then
    QEMU_PATH="../../qemu-libafl-asp/"
fi
if [ ! -d $QEMU_PATH ]
then
    echo "$QEMU_PATH is not a valid directory"
fi

$QEMU_PATH/build/arm-softmmu/qemu-system-arm \
    --singlestep \
    --machine amd-psp-$ZEN_GENERATION \
    --nographic \
    -global amd-psp.dbg_mode=true \
    -device loader,file=$ROM_BL,addr=0xffff0000,force-raw=on \
    -global driver=amd_psp.smnflash,property=flash_img,value=$ASP_FLASH_IMAGE \
    -bios $ASP_FLASH_IMAGE \
    -S -gdb tcp::$PORT \
    -d unimp,guest_errors,mmu,trace:psp_*,trace:ccp_*
