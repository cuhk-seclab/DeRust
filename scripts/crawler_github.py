"""Script that crawls crates from Github."""

import requests
import os
import pickle
from subprocess import Popen, DEVNULL

# Global variable that stores the repository addresses that have been processed
repo_set = set()

def make_repo_list():
    """Return a list of repositories from GitHub API."""
    request_page = requests.get('https://api.github.com/repositories')
    repositories = []
    page = 1
    per_page = 100
    while True:
        url = f'https://api.github.com/search/repositories?q=language:rust&sort=stars&order=desc&page={page}&per_page={per_page}'
        response = requests.get(url)
        results = response.json()
        if not results:
            break
        page += 1
    return repositories


if __name__=='__main__':
    count = 0  # The number of Github repos successfully cloned
    print("Requesting the API of github...")
    repo_list = make_repo_list()  # uncomment this to update the crate list
    # print("Loading repo list...")
    # repo_list = load_repo_list()
    # print("Got addresses of {} repos, start cloning...".format(
    #     len(repo_list)))
    # for repo in repo_list:
    #     if count >= 200:
    #         break

    #     # if clone_repo(name, repo):
    #     #     count += 1

    # print(count, "repos cloned")