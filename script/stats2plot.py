import re
import os
import seaborn as sns
import matplotlib.pyplot as plt
from matplotlib.ticker import MaxNLocator
from pathlib import Path
import argparse

sns.set_theme(style="white", font_scale=1.7)

parser = argparse.ArgumentParser(description='Plot libafl log file')
parser.add_argument("run_dir", type=str, help="Path to the specific fuzzer run (not the general runs/ dir)")
parser.add_argument("-p", "--plot_file", type=str, default="stats", help="File name to write the plot to")
args = parser.parse_args()

run_dir_path = Path(args.run_dir)

log_dir_path = os.path.join(run_dir_path, Path("logs"))
if not os.path.isdir(log_dir_path):
    print(f"{log_path} no a valid path")
    exit(1)


stats_re = re.compile(
    "run time: ([0-9]{1,10})h-([0-9]{1,2})m-([0-9]{1,2})s, clients: ([0-9]{1,10}), corpus: ([0-9]{1,10}), objectives: ([0-9]{1,10}), executions: ([0-9]{1,20}), exec/sec: ([0-9]{1,10})"
)
stats_start_re = re.compile("\[Stats #0\]")
stats_start_multicore_re = re.compile("(GLOBAL)")
stats_runtime_re = re.compile("run time: ([0-9]{1,10})h-([0-9]{1,2})m-([0-9]{1,2})s")
stats_client_re = re.compile("clients: ([0-9]{1,10})")
stats_corpus_re = re.compile("corpus: ([0-9]{1,10})")
stats_objectives_re = re.compile("objectives: ([0-9]{1,10})")
stats_executions_re = re.compile("executions: ([0-9]{1,20})")
stats_execsec_re = re.compile("exec/sec: ([0-9]{1,10})")
stats_edges_re = re.compile("edges: ([0-9]{1,5})")

libafl_log_path = Path(os.path.join(log_dir_path, Path("libafl.log")))
if not os.path.isfile(libafl_log_path):
    print(f"{libafl_log_path} is not a valid file")
    exit(1)

stats = {
    "runtime": [],
    "sec": [],
    "corpus": [],
    "objectives": [],
    "executions": [],
    "exec/sec": [],
    "edges": [],
}

with open(libafl_log_path, "r") as f:
    next_timestep = 10
    last_timestep = 0
    last_max_edges = 0
    for line in f:
        stats_start_re_result = re.match(stats_start_re, line)
        stats_start_multicore_re_result = re.search(stats_start_multicore_re, line)
        if stats_start_re_result or stats_start_multicore_re_result:
                stats_runtime_re_res = re.search(stats_runtime_re, line)
                total_sec = int(stats_runtime_re_res[3]) + 60*int(stats_runtime_re_res[2]) + 3600*int(stats_runtime_re_res[1])
                if total_sec >= 60 or total_sec > last_timestep:
                    if total_sec >= next_timestep:
                        stats_client_re_res = re.search(stats_client_re, line)
                        stats_corpus_re_res = re.search(stats_corpus_re, line)
                        stats_objectives_re_res = re.search(stats_objectives_re, line)
                        stats_executions_re_res = re.search(stats_executions_re, line)
                        stats_execsec_re_res = re.search(stats_execsec_re, line)
                        stats_edges_re_res = re.search(stats_edges_re, line)

                        stats["runtime"].append(f"{stats_runtime_re_res[1]}h-{stats_runtime_re_res[2]}m-{stats_runtime_re_res[3]}s")
                        stats["sec"].append(next_timestep)
                        stats["corpus"].append(int(stats_corpus_re_res[1]))
                        stats["objectives"].append(int(stats_objectives_re_res[1]))
                        stats["executions"].append(int(stats_executions_re_res[1]))
                        stats["exec/sec"].append(int(stats_execsec_re_res[1]))
                        stats["edges"].append(last_max_edges)

                        last_timestep = total_sec
                        if next_timestep < 3600:
                            next_timestep += 10
                        else:
                            next_timestep += 600
        else:
            stats_edges_re_res = re.search(stats_edges_re, line)
            if stats_edges_re_res:
                new_edges = int(stats_edges_re_res[1])
                if new_edges > last_max_edges:
                    last_max_edges = new_edges

def stats2plot(stats: dict):
    t = stats["sec"]
    data3 = stats["edges"]
    data2 = stats["objectives"]
    data1 = stats["exec/sec"]
    linewidth = 3
    fontsize = 26

    fig, ax1 = plt.subplots()
    fig.subplots_adjust(right=0.75)
    fig.set_figwidth(20)
    fig.set_figheight(8)

    color = sns.color_palette(palette=None)[0]
    ax1.set_xlabel('time in s', fontsize=fontsize)
    ax1.yaxis.set_major_locator(MaxNLocator(integer=True))
    ax1.set_ylabel("exec/sec", color=color, fontsize=fontsize)
    ax1.plot(t, data1, color=color, linewidth=linewidth)
    ax1.tick_params(axis='y', labelcolor=color)

    ax2 = ax1.twinx()
    color = sns.color_palette(palette=None)[1]
    ax2.set_ylabel("objectives", color=color, fontsize=fontsize)
    ax2.plot(t, data2, color=color, linewidth=linewidth)
    ax2.set_ylim(0.0, max(data2) if max(data2) >= 1.0 else 1.0)
    ax2.tick_params(axis='y', labelcolor=color)

    ax3 = ax1.twinx()
    ax3.spines["right"].set_position(("axes", 1.1))
    color = sns.color_palette(palette=None)[2]
    ax3.set_ylabel("edges", color=color, fontsize=fontsize)
    ax3.plot(t, data3, color=color, linewidth=linewidth)
    ax3.tick_params(axis='y', labelcolor=color)

    # otherwise the right y-label is slightly clipped
    fig.tight_layout()

    plot_file = Path(args.plot_file+".png")
    print(f"Plot written to {Path(libafl_log_path.parent,plot_file)}")
    plt.savefig(Path(libafl_log_path.parent, plot_file), bbox_inches='tight')

stats2plot(stats)
