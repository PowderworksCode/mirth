import json, subprocess, os, re, time
env = {**os.environ, 'GH_HOST': 'github.com'}
d = json.load(open('issues.json'))
Q = '''query($n:Int!) { repository(owner:"rust-lang", name:"rust") { pullRequest(number:$n) { body } } }'''
Q2 = '''query($n:Int!) { repository(owner:"rust-lang", name:"rust") { pullRequest(number:$n) { number title body merged files(first:40) { nodes { path } } } } }'''
def gql(q, n):
    r = subprocess.run(['gh', 'api', 'graphql', '-f', f'query={q}', '-F', f'n={n}'], capture_output=True, text=True, env=env)
    return json.loads(r.stdout)['data']['repository']['pullRequest']
cache = {}
fixed = 0
for x in d:
    if not x['pr_title'].startswith('Rollup'): continue
    if x['pr'] not in cache:
        body = gql(Q, x['pr'])['body'] or ''
        cache[x['pr']] = sorted(set(int(m) for m in re.findall(r'#(\d{5,6})', body)))
    for n in cache[x['pr']]:
        key = ('sub', n)
        if key not in cache:
            cache[key] = gql(Q2, n)
        p = cache[key]
        if p and re.search(rf'#{x["issue"]}\b|issues/{x["issue"]}\b', p['body'] or ''):
            x.update(pr=p['number'], pr_title=p['title'], pr_body=(p['body'] or '')[:2000],
                     pr_files=[f['path'] for f in p['files']['nodes']])
            fixed += 1
            break
json.dump(d, open('issues.json', 'w'))
print('unrolled', fixed, 'still rollup', sum(x['pr_title'].startswith('Rollup') for x in d))
