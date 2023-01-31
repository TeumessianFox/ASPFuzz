import glob
import re
import os
from pathlib import Path
import argparse

parser = argparse.ArgumentParser(description='Analysing solution metadata files')
parser.add_argument("run_dir", type=str, help="Path to the specific fuzzer run (not the general runs/ dir)")
parser.add_argument('-p', '--uninteresting_pc', nargs='+', default=[], help="Don't output solutions with these pc's")
args = parser.parse_args()

run_dir_path = Path(args.run_dir)
uninteresting_pc = args.uninteresting_pc

hash_re = re.compile("/.([0-9a-f\-]{16,19}).metadata")
pc_re = re.compile("pc: (0x[0-9a-f]*)")
lr_re = re.compile("lr: (0x[0-9a-f]*)")

solutions_path = os.path.join(run_dir_path, Path("solutions"))
if not os.path.isdir(solutions_path):
    print(f"{solutions_path} no a valid path")
    exit(1)

metadata_dict = {}
file_counter = 0
for metadata_file in glob.glob(os.path.join(solutions_path, ".*.metadata")):
    file_counter += 1
    hash_re_result = re.search(hash_re, metadata_file)
    if hash_re_result:
        hash_val = hash_re_result[1]
        with open(metadata_file, 'r') as f:
            lines = [line.rstrip().replace('"', '').strip() for line in f]
        for line in lines:
            pc_out = re.match(pc_re, line)
            lr_out = re.match(lr_re, line)
            if pc_out:
                pc = pc_out[1]
            if lr_out:
                lr = lr_out[1]
        metadata_dict[hash_val] = {"pc": pc, "lr": lr}
    else:
        print(f"Could not parse {metadata_file}")

print(f"\nAnalyzed {file_counter} metadata files:\n")

for meta_key, meta_val in metadata_dict.items():
    if not meta_val["pc"] in uninteresting_pc:
        print(f"{meta_key}:")
        print(f"\tpc: {meta_val['pc']}")
        print(f"\tlr: {meta_val['lr']}")
