import glob
from pathlib import Path
import os
import sys
import argparse
import yaml
from psptool.utils import fletcher32

parser = argparse.ArgumentParser(description='Creating full flash images from fuzzer solutions')
parser.add_argument("run_dir", type=str, help="Path to the specific fuzzer run (not the general runs/ dir)")
parser.add_argument("-n", "--new_dir", type=str, default="full_img/", help="Directory to write the new flash images to")
args = parser.parse_args()

run_dir_path = Path(args.run_dir)
solutions_dir_path = os.path.join(run_dir_path, Path("solutions"))
yaml_path = os.path.join(run_dir_path, Path("config.yaml"))
new_dir_path = Path(args.new_dir)

COMBO_DIR_MAGIC         = 0x50535032
COMBO_DIR_ADDR          = 0x000c0000
DIR_ADDR                = 0x000d1000
DIR_MAGIC_OFFSET        = 0x0
DIR_CHECKSUM_OFFSET     = 0x4
DIR_LEN_OFFSET          = 0x8
COMBO_DIR_MAGIC_ADDR    = COMBO_DIR_ADDR + DIR_MAGIC_OFFSET
COMBO_DIR_CHECKSUM_ADDR = COMBO_DIR_ADDR + DIR_CHECKSUM_OFFSET
COMBO_DIR_LEN_ADDR      = COMBO_DIR_ADDR + DIR_LEN_OFFSET
DIR_MAGIC_ADDR          = DIR_ADDR + DIR_MAGIC_OFFSET
DIR_CHECKSUM_ADDR       = DIR_ADDR + DIR_CHECKSUM_OFFSET
DIR_LEN_ADDR            = DIR_ADDR + DIR_LEN_OFFSET

# Load yaml file if the file path is valid
if not os.path.isfile(yaml_path):
    print(f"{yaml_path}: Not a valid file!")
with open(yaml_path, 'r') as stream:
    try:
        yaml_config = yaml.safe_load(stream)
    except yaml.YAMLError as exc:
        print(exc)

# Check if solutions directory path exists
if not os.path.isdir(solutions_dir_path):
    print(f"{solutions_dir_path}: Not a valid directory!")

flashimg_path = os.path.join(Path("../fuzzer/amd_sp/"), Path(yaml_config["flash"]["base"]))
if not os.path.isfile(flashimg_path):
    print(f"{flashimg_path}: Not a valid file!")
with open(flashimg_path, "rb") as f:
    flashimg_bytes = f.read(-1)

file_counter = 0
transformation_counter = 0
print("")
new_flashimg_dir_path =  os.path.join(solutions_dir_path, new_dir_path)
Path(new_flashimg_dir_path).mkdir(parents=True, exist_ok=True)
for solutions_file_path in glob.glob(os.path.join(solutions_dir_path,"*")):
    if not os.path.isfile(solutions_file_path):
        continue
    file_counter += 1
    print(f"{solutions_file_path}\t---> ", end="")
    with open(solutions_file_path, 'r+b') as f:
        solutions_bytes = f.read(-1)
    mut_flashimg_bytes = bytearray(flashimg_bytes)

    for mem_area in yaml_config["input"]["mem"]:
        mem_addr = mem_area["addr"]
        mem_size = mem_area["size"]
        if mem_addr == None or mem_size == None:
            break

        # Zeroing flash image section
        mut_flashimg_bytes[mem_addr:mem_addr+mem_size] = bytearray(mem_size)
        # Writing corpus to flash image section
        if len(solutions_bytes) < mem_size:
            mut_flashimg_bytes[mem_addr:mem_addr+len(solutions_bytes)] = solutions_bytes
            corpus_bytes = solutions_bytes[len(solutions_bytes):]
        else:
            mut_flashimg_bytes[mem_addr:mem_addr+mem_size] = solutions_bytes[0:mem_size]
            solutions_bytes = solutions_bytes[mem_size:]

    for fixed_mem in yaml_config["input"]["fixed"]:
        fixed_addr = fixed_mem["addr"]
        fixed_val = fixed_mem["val"]
        if fixed_addr == None or fixed_val == None:
            break

        mut_flashimg_bytes[fixed_addr:fixed_addr+4] = fixed_val.to_bytes(4, 'little')

    if int.from_bytes(mut_flashimg_bytes[COMBO_DIR_MAGIC_ADDR:COMBO_DIR_MAGIC_ADDR+4], 'little') == COMBO_DIR_MAGIC:
        # Fletcher checksum 1
        dir_len_1 = int.from_bytes(mut_flashimg_bytes[COMBO_DIR_LEN_ADDR:COMBO_DIR_LEN_ADDR+4],'little') * 16 + 0x18
        dir_data_1 = mut_flashimg_bytes[COMBO_DIR_LEN_ADDR:COMBO_DIR_LEN_ADDR+dir_len_1]
        mut_flashimg_bytes[COMBO_DIR_CHECKSUM_ADDR:COMBO_DIR_CHECKSUM_ADDR+4] = fletcher32(dir_data_1)

        # Fletcher checksum 2
        dir_len_2 = int.from_bytes(mut_flashimg_bytes[DIR_LEN_ADDR:DIR_LEN_ADDR+4],'little') * 16 + 0x8
        dir_data_2 = mut_flashimg_bytes[DIR_LEN_ADDR:DIR_LEN_ADDR+dir_len_2]
        mut_flashimg_bytes[DIR_CHECKSUM_ADDR:DIR_CHECKSUM_ADDR+4] = fletcher32(dir_data_2)
    else:
        # Fletcher checksum 1
        dir_len_1 = int.from_bytes(mut_flashimg_bytes[COMBO_DIR_LEN_ADDR:COMBO_DIR_LEN_ADDR+4],'little') * 16 + 0x8
        dir_data_1 = mut_flashimg_bytes[COMBO_DIR_LEN_ADDR:COMBO_DIR_LEN_ADDR+dir_len_1]
        mut_flashimg_bytes[COMBO_DIR_CHECKSUM_ADDR:COMBO_DIR_CHECKSUM_ADDR+4] = fletcher32(dir_data_1)

    # Write to new file
    with open(os.path.join(new_flashimg_dir_path, solutions_file_path[-16:] + "_full.ROM"), 'wb') as f:
        print(f'{os.path.join(new_flashimg_dir_path, solutions_file_path[-16:] + "_full.ROM")}')
        f.write(mut_flashimg_bytes)
    transformation_counter += 1

print(f"\n{transformation_counter}/{file_counter} solution files have been transformed")
