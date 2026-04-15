"""Script that runs evaluation in parallel."""

import os
import sys
import signal
import subprocess
import threading
from multiprocessing.pool import ThreadPool
import csv
import re
import time



class bcolors:
    """Define colors for pretty outputs."""
    HEADER = '\033[95m'

    OKRED = '\033[31m'
    OKBLUE = '\033[94m'
    OKGREEN = '\033[92m'
    OKPURPLE = '\033[95m'
    OKCYAN = '\033[96m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'

    ENDC = '\033[0m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'


# path to the current script
# root_dir = os.path.dirname(os.path.abspath(__file__))
root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__))) # BugChecker/
# path to the output directory
output_dir = os.path.join(root_dir, "./outputs")
# path to the cargo sub-command
executable = os.path.join(root_dir, "./target/release/cargo-derust")

# Lock for the global counters
lock = threading.Lock()
count = 0


def cargo_clean(crate_dir):
    """Run `cargo clean` to make sure it does not use cache."""
    crate_name = os.path.basename(crate_dir)
    print("Cleaning up", crate_name)
    subprocess.Popen(["cargo", "clean"], cwd=crate_dir).wait()


def evaluate(crate_dir):
    """Evaluate a crate given its path."""

    crate_name = os.path.basename(crate_dir)
    print("Evaluating", crate_name)

    if not os.path.exists(os.path.join(crate_dir, "Cargo.toml")):
        print("This seems to be a workspace: ", crate_dir, ", which is not a crate directory")
        write_result_to_file(crate_name, ["", 0, 0, 0, "fail"])
        return ["", 0, 0, 0, "fail"]    


    # Run `cargo clean` to make sure it does not use cache
    # subprocess.Popen(["cargo", "clean"], cwd=crate_dir).wait()

    # Use `time` command to get execution time and peak memory usage
    with subprocess.Popen(
        ["/usr/bin/time", "-f", "%M\n%e", executable, "derust"],
            cwd=crate_dir,
            stdout=subprocess.PIPE,
            # stderr=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            preexec_fn=os.setsid) as process:
        try:
            # Analysis results are in stdout, execution time etc. are in stderr
            out, err = process.communicate(timeout=timeout_sec)

            out_str = out.decode("utf-8")
            err_str = err.decode("utf-8").split()
            bug_count = 0
            status = ""

            out_lines = out_str.splitlines()
            for out_line in out_lines:
                if 'The overall detected bug number is:' in out_line:
                    bug_count = out_line.split(': ')[-1].strip()

            if out_str != "":
                print(bcolors.OKPURPLE, "Successfully run into this crate", bcolors.ENDC)
                print(bcolors.OKBLUE, crate_name, bcolors.ENDC)
                print(bcolors.OKRED, bug_count, bcolors.ENDC)
                # print(bcolors.OKGREEN, out_str, bcolors.ENDC)

            elasp_time = float(err_str[-1])
            peak_mem = int(err_str[-2])

            if lock.acquire():
                global count
                count += 1
                print("Progress:", count, "/", total_count)
                lock.release()

            if process.returncode == 0:
                status = "success"
                print(bcolors.OKGREEN, "Finish analyzing crate", crate_name,
                      bcolors.ENDC)
                print("Cleaning up")
                subprocess.Popen(["cargo", "clean"], cwd=crate_dir).wait()
                write_result_to_file(crate_name, [out_str, elasp_time, peak_mem, bug_count, status])
                return [out_str, elasp_time, peak_mem, bug_count, status]

            else:
                status = "fail"
                print(bcolors.FAIL, "Error while analyzing crate", crate_name,
                      bcolors.ENDC)
                print("Cleaning up")
                subprocess.Popen(["cargo", "clean"], cwd=crate_dir).wait()
                write_result_to_file(crate_name, [out_str, elasp_time, peak_mem, bug_count, status])
                return [out_str, elasp_time, peak_mem, bug_count, status]

        except subprocess.TimeoutExpired:
            print(bcolors.FAIL, "Timeout while analyzing crate", crate_name,
                  bcolors.ENDC)

            if lock.acquire():
                count += 1
                print("Progress:", count, "/", total_count)
                lock.release()
            # send signal to the process group
            os.killpg(process.pid, signal.SIGTERM)

            print("Cleaning up")
            subprocess.Popen(["cargo", "clean"], cwd=crate_dir).wait()
            write_result_to_file(crate_name, ["", 0, 0, 0, "timeout"])
            return ["", 0, 0, 0, "timeout"]


def mkdir(dir_name):
    """Create a directory (if it does not exist) in the current directory."""
    if not os.path.exists(dir_name):
        os.makedirs(dir_name)


def write_result_to_file(crate_name, result):
    """Write the result to a file."""
    f_dir = os.path.join(os.path.join(crate_dir, "analysis_logs"), crate_name)
    f_path = f_dir + ".txt"
    f = open(f_path, "w")
    f.write(crate_name + ":\n")
    f.write(str(result[0]) + "\n\n")
    f.write("Time cost: " + str(result[1]) + "\n\n")
    f.write("Memory cost: " + str(result[2]) + "\n\n")
    f.write("Bug number: " + str(result[3]) + "\n\n")
    f.write("Status: " + str(result[4]) + "\n\n")
    f.close()


if __name__ == "__main__":

    if len(sys.argv) != 5:
        print(
            "Need four arguments to specify the crate list, the crate directory, the size of the thread pool, and the timeout in seconds"
        )
        print(
            "Usage example: `python evaluate.py crate_list.txt ./crates 8 240`"
            "Usage example2: `python evaluate.py experiments_list.txt ./experiments 1 240`"
        )
        exit(1)

    # Read crate list that will be analyzed
    crate_list_file = open(sys.argv[1], 'r').readlines()
    crate_list = [line.strip() for line in crate_list_file]
    total_count = len(crate_list)
    print(crate_list)

    # path to the crate directory
    crate_dir = os.path.join(root_dir, sys.argv[2])
    # paths to the all test cases
    test_cases_dir = [os.path.join(crate_dir, i) for i in crate_list]

    # Read the size of the thread pool
    num_thread = int(sys.argv[3])
    print(total_count, "tasks in total, run in", num_thread, "threads")

    # Read the timeout limit
    timeout_sec = int(sys.argv[4])

    # mkdir(output_dir)
    # os.chdir(output_dir)


    # Cargo clean all the crates first
    # p = ThreadPool(num_thread)
    # for i in range(0, total_count):
    #     p.apply_async(cargo_clean, args=(test_cases_dir[i], ))
    # p.close()
    # p.join()


    results = []

    # Run evaluation in parallel
    p = ThreadPool(num_thread)
    for i in range(0, total_count):
        results.append(p.apply_async(evaluate, args=(test_cases_dir[i], )))
    p.close()
    p.join()
    results = {crate_list[i]: r.get() for i, r in enumerate(results)}


    # Dump results in CSV format
    with open('result.csv', 'w', newline='') as csvfile:
        csvwriter = csv.writer(csvfile,
                               delimiter=',',
                               quotechar='|',
                               quoting=csv.QUOTE_MINIMAL)
        csvwriter.writerow(["Package", "bug_number", "time", "memory", "status"])
        for k, v in results.items():
            csvwriter.writerow([
                k,
                str(v[3]),
                str(v[1]),
                str(v[2] / 1024),
                str(v[4])
            ])

    # Print and dump result
    for k, v in results.items():
        f_dir = os.path.join(os.path.join(crate_dir, "analysis_all_logs"), k)
        f_path = f_dir + ".txt"
        
        f = open(f_path, "w")
        f.write(k + ":\n")
        f.write(str(v[0]) + "\n\n")
        f.write("Time cost: " + str(v[1]) + "\n\n")
        f.write("Memory cost: " + str(v[2]) + "\n\n")
        f.write("Bug number: " + str(v[3]) + "\n\n")
        f.write("Status: " + str(v[4]) + "\n\n")
        f.close()
