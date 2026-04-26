import os, json, urllib.request

# Hardcoding the repo since Cloudflare doesn't set it the same way GH Actions does
repo  = "1minds3t/uv-ffi"
token = os.environ.get("GH_TOKEN")

def gh(url):
    headers = {"Accept": "application/vnd.github+json"}
    if token:
        headers["Authorization"] = f"token {token}"
    req = urllib.request.Request(url, headers=headers)
    return json.loads(urllib.request.urlopen(req).read())

def pypi_wheels(pkg):
    try:
        req  = urllib.request.Request(f"https://pypi.org/pypi/{pkg}/json")
        data = json.loads(urllib.request.urlopen(req).read())
        return [(v, f["filename"], f["url"])
                for v, files in data["releases"].items()
                for f in files if f["filename"].endswith(".whl")]
    except Exception as e:
        print(f"PyPI fetch failed: {e}")
        return[]

# 1. Create the output directories
os.makedirs("_site/uv-ffi", exist_ok=True)

gh_wheels =[]
page = 1
while True:
    rels = gh(f"https://api.github.com/repos/{repo}/releases?per_page=50&page={page}")
    if not rels:
        break
    for rel in rels:
        assets_pg2 = 1
        while True:
            assets_url = f"https://api.github.com/repos/{repo}/releases/{rel['id']}/assets?per_page=100&page={assets_pg2}"
            batch = gh(assets_url) or []
            for a in batch:
                if a["name"].endswith(".whl"):
                    gh_wheels.append((rel["tag_name"], a["name"], a["browser_download_url"]))
            if len(batch) < 100:
                break
            assets_pg2 += 1
    page += 1

pip_wheels = pypi_wheels("uv-ffi")

seen = {}
for tag, name, url in pip_wheels:
    seen[name] = (tag, name, url)
for tag, name, url in gh_wheels:
    seen[name] = (tag, name, url)

all_wheels = sorted(seen.values(), key=lambda x: (x[0], x[1]))

links = "\n".join(
    f'  <a href="{url}" data-requires-python=">=3.8">{name}</a><br>'
    for _, name, url in all_wheels
)

# 2. Write the HTML files
with open("_site/uv-ffi/index.html", "w") as fh:
    fh.write(f"<!DOCTYPE html>\n<html>\n  <head><title>Links for uv-ffi</title></head>\n"
             f"  <body>\n    <h1>Links for uv-ffi</h1>\n{links}\n  </body>\n</html>\n")

with open("_site/index.html", "w") as fh:
    fh.write(f"<!DOCTYPE html>\n<html>\n  <head><title>uv-ffi wheel index</title></head>\n"
             f"  <body>\n    <h1>uv-ffi wheel index</h1>\n"
             f'    <p>Mainstream wheels: <a href="https://pypi.org/project/uv-ffi/">PyPI</a></p>\n'
             f"    <p>Exotic/extended platform wheels hosted here.</p>\n"
             f"    <pre>pip install uv-ffi --extra-index-url https://1minds3t.github.io/uv-ffi/</pre>\n"
             f'    <p><a href="uv-ffi/">Browse all {len(all_wheels)} wheels</a></p>\n'
             f"  </body>\n</html>\n")

print(f"Indexed {len(all_wheels)} total wheels")
