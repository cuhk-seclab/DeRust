"""Script that crawls crates from crates.io."""

import requests
import os
import pickle

from subprocess import Popen, DEVNULL

# Global variable that stores the repository addresses that have been processed
repo_set = set()


def clone_repo(name, repo):
    """Clone a repository from address `repo` and rename the directory as `name`.

    Return `True` if succeed, `False` if failed.
    """
    # crates.io API may not always correctly return the repository address
    if repo is None:
        print("Warning:", name, "is ignored because its repository address is none")
        return False

    global repo_set
    if repo in repo_set:
        # Different crates on crates.io may have the same repository address
        # Do not clone repositories that have already been cloned
        print("Warning:", name, "is ignored because it has already been cloned from", repo)
        return False
    else:
        repo_set.add(repo)
        print("Cloning repo: ", name, "from: ", repo)
        my_env = os.environ.copy()
        my_env[
            "GIT_TERMINAL_PROMPT"] = "0"  # Some repositories need username and password, use this to fail instead of prompting for credentials
        p = Popen(["git", "clone", "--depth=1", repo, name],
                  cwd="../crates",
                  stdout=DEVNULL,
                  stderr=DEVNULL,
                  env=my_env)
        p.communicate()[0]
        if p.returncode != 0:
            print("Warning: Error whiling cloning repo:", repo)
            return False
        return True


def make_crate_list():
    """Return a list of crates according to the category and page lists."""
    # category_str = "" if category == "" else "category=" + category
    request_page = requests.get('https://crates.io/api/v1/crates')
    crate_total_num = int(request_page.json()['meta']['total'])
    # 100 crates per page, take the ceiling
    page_list = list(range(1, (crate_total_num + 100 - 1) // 100 + 1))
    crate_list = []
    count = 0
    for page in page_list:
        if count > 1000: # Crawl 100k crates
            break
        request_page = requests.get(
            'https://crates.io/api/v1/crates?&page={}&per_page=100&sort=downloads'
            .format(page))
        crate_list += request_page.json()['crates']
        print("Count:", count)
        count += 1
    f = open("crate_list.pickle", "wb")
    pickle.dump(crate_list, f)
    f.close()
    return crate_list


def load_crate_list():
    """Load the crate list."""
    f = open("crate_list.pickle", "rb")
    crate_list = pickle.load(f)
    f.close()
    return crate_list


def dump_crate_list(crate_list):
    """Dump the crate list to a file."""
    f = open("crates_all.txt", "w")
    count = 0
    for crate in crate_list:
        # if count >= 10:
        #     break
        f.write(crate['name'] + "\n")
        count += 1
    f.close()


def dumo_crate_list2():
    """Travel the crates directory and dump the crate list to a file."""
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    crate_dir = os.path.join(root_dir, "./crates")

    with open("crate_list.txt", "w") as file:
        count = 0
        entries = os.listdir(crate_dir)
        sorted_entries = sorted(entries)
        for entry in sorted_entries:
            entry_path = os.path.join(crate_dir, entry)
            if os.path.isdir(entry_path):
                file.write(entry + "\n")
                print("Count: ", count)
                count += 1


def should_ignore(name, description):
    """Determine whether a crate should be ignored according to its name and description."""
    description = description.lower()
    # Exclude crates that are related to FFI, macro/trait definitions, multi-threads, etc.
    # keywords = [
    #     "ffi", "macro", "binding", "wrapper", "float", "api", "abi", "trait",
    #     "concurrent", "async", "pin", "mutex", "lock", "atomic", "thread",
    #     "string", "rational", "libm", "cortex", "hal", "simd", "asm", "sys",
    #     "stm32", "arch", "gpio"
    # ]
    keywords = [
        "ffi", "C", "C++", "macro", "trait",
        "concurrent", "async", "pin", "mutex", "lock", "atomic", "thread"
    ]
    if any([keyword in name + description for keyword in keywords]):
        print("Warning:", name, "is ignored because it is not our concern")
        return True
    return False


# Count crates number: ls -l | grep "^d" | wc -l; ls -l | grep "^-" | wc -l
if __name__ == '__main__':
    count = 0  # The number of crates successfully cloned

    # 1) Crawl crate list from crates.io
    # print("Requesting the API of crates.io...")
    # crate_list = make_crate_list()  # uncomment this to update the crate list
    # crate_list = make_crate_list("no-std")

    # 2) Load and dump the crate list
    print("Loading crate list...")
    crate_list = load_crate_list()
    # dump_crate_list(crate_list)
    # dumo_crate_list2()

    # crate_list = []
    # with open("crates_all_10k.txt", "r") as f:
    #     crate_list = f.readlines()
    #     crate_list = [crate.strip() for crate in crate_list]

    # ipdb.set_trace()

    # ) Load cloned crates
    crate_cloned_list_1 = []
    with open("crate_list_round1.txt", "r") as f:
        crate_cloned_list_1 = f.readlines()
        crate_cloned_list_1 = [crate.strip() for crate in crate_cloned_list_1]
    crate_cloned_list_2 = []
    with open("crate_list_round2.txt", "r") as f:
        crate_cloned_list_2 = f.readlines()
        crate_cloned_list_2 = [crate.strip() for crate in crate_cloned_list_2]
    black_list = []
    with open("black_list.txt", "r") as f:
        black_list = f.readlines()
        black_list = [crate.strip() for crate in black_list]

    # 3) Clone repositories
    print("Got addresses of {} crates, start cloning...".format(len(crate_list)))
    f = open("crate_list.txt", "w")
    # filter = 0
    for crate in crate_list:
        # filter += 1
        # print("Filter: ", filter)
        # if filter < 50000:
        #     continue

        if count >= 1000:
            break
        name = crate['name']
        description = crate['description']
        repo = crate['repository']

        if "solana" in name:
            print("Warning:", name, "is ignored because it is not our concern")
            continue
        if name in black_list:
            print("Warning:", name, "is ignored because it is in the black list")
            continue
        if name in crate_cloned_list_1:
            # print("Warning:", name, "is ignored because it has already been cloned")
            # continue
            if not should_ignore(name, description):
                if clone_repo(name, repo):
                    f.write(crate['name'] + "\n")
                    print("Count: ", count)
                    count += 1

        # if name in crate_cloned_list_2:
        #     print("Warning:", name, "is ignored because it has already been cloned")
        #     continue

        # if not should_ignore(name, description):
        #     if clone_repo(name, repo):
        #         f.write(crate['name'] + "\n")
        #         print("Count: ", count)
        #         count += 1
    f.close()
    print(count, "crates cloned")