import glob
from pathlib import Path
import os
import argparse

parser = argparse.ArgumentParser(description='Check fuzzer solutions for known buffer overflow')
parser.add_argument("run_dir", type=str, help="Path to the specific fuzzer run (not the general runs/ dir)")
args = parser.parse_args()

HEADER_LEN_FIELD_OFFSET = 0x14
COMBO_DIR_MAGIC         = 0x50535032
COMBO_DIR_ADDR          = 0x000c0000
DIR_ADDR                = 0x000d1000
NORMAL_BL_MAGIC         = 0x1
RECOVERY_BL_MAGIC       = 0x3

# Check if run directory path exists
run_dir_path = Path(args.run_dir)
if not os.path.isdir(run_dir_path):
    print(f"{run_dir_path}: Not a valid directory!")

# Check if solutions directory path exists
solutions_dir_path = os.path.join(run_dir_path, Path("solutions"))
if not os.path.isdir(solutions_dir_path):
    print(f"{solutions_dir_path}: Not a valid directory!")

# Check if full_img solutions directory path exists
full_img_solutions_dir_path = os.path.join(solutions_dir_path, Path("full_img"))
if not os.path.isdir(full_img_solutions_dir_path):
    print(f"{full_img_solutions_dir_path}: Not a valid directory!")

print(f"This script searches for the known buffer overflow in the solutions files.")
print(f"The DIR header is assumed to be at {DIR_ADDR:#010x}.")
print("")

file_counter = 0
known_counter = 0
unknown_counter = 0
for solutions_file_path in glob.glob(os.path.join(full_img_solutions_dir_path,"*")):
    file_counter += 1
    if not os.path.isfile(solutions_file_path):
        continue
    with open(solutions_file_path, 'r+b') as f:
        solutions_bytes = f.read(-1)

    found = False
    if int.from_bytes(solutions_bytes[COMBO_DIR_ADDR:COMBO_DIR_ADDR+4], 'little') == COMBO_DIR_MAGIC:
        dir_addr = DIR_ADDR
    else:
        dir_addr = COMBO_DIR_ADDR
    for i in range(0, 64):
        dir_entry_addr = dir_addr + 0x10 + i * 0x10
        dir_entry_magic = int.from_bytes(solutions_bytes[dir_entry_addr:dir_entry_addr + 0x4], 'little')
        #print(f"Entry magic = {dir_entry_magic:#010x}")
        if dir_entry_magic == RECOVERY_BL_MAGIC or dir_entry_magic == NORMAL_BL_MAGIC:
            entry_addr = int.from_bytes(solutions_bytes[dir_entry_addr + 0x8:dir_entry_addr + 0xc], 'little') & 0x00ffffff
            #print(f"Entry address = {entry_addr:#010x}")
            entry_len_addr = entry_addr + HEADER_LEN_FIELD_OFFSET
            entry_len = int.from_bytes(solutions_bytes[entry_len_addr:entry_len_addr + 0x4], 'little')
            if entry_len >= 0x80000000:
                known_counter += 1
                if dir_entry_magic == NORMAL_BL_MAGIC:
                    print(f"Found in {solutions_file_path} at on-chip bootloader: {entry_len_addr:#010x}\n")
                elif dir_entry_magic == RECOVERY_BL_MAGIC:
                    print(f"Found in {solutions_file_path} at recovery bootloader: {entry_len_addr:#010x}\n")
                found = True
                break
    if found:
        continue
    print(f"{solutions_file_path}: unkown solutions\n")
    unknown_counter += 1

print("")
print(f"#ROMFiles = {file_counter}")
print(f"#NumKnown = {known_counter}")
print(f"#NumUnknown = {unknown_counter}")
