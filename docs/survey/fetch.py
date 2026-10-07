import json, subprocess, sys
Q = '''query($q:String!, $after:String) { search(query:$q, type:ISSUE, first:50, after:$after) {
  issueCount pageInfo { hasNextPage endCursor }
  nodes { ... on Issue { number title closedAt body
    labels(first:15) { nodes { name } }
    timelineItems(itemTypes:[CLOSED_EVENT], last:1) { nodes { ... on ClosedEvent {
      closer { ... on PullRequest { number title body merged files(first:40) { nodes { path } } }
               ... on Commit { associatedPullRequests(first:1) { nodes { number title body merged files(first:40) { nodes { path } } } } } } } } } } } } }'''
import datetime
out = []
end = datetime.date(2026, 10, 7)
while len(out) < 1000 and end.year > 2020:
  start = end - datetime.timedelta(days=30)
  q = f"repo:rust-lang/rust is:issue is:closed reason:completed label:C-bug closed:{start}..{end}"
  after = None
  while True:
      args = ['gh', 'api', 'graphql', '-f', f'query={Q}', '-f', f'q={q}']
      if after: args += ['-f', f'after={after}']
      r = json.loads(subprocess.run(args, capture_output=True, text=True, env={**__import__('os').environ, 'GH_HOST': 'github.com'}).stdout)
      s = r['data']['search']
      for n in s['nodes']:
          closers = [t['closer'] for t in n['timelineItems']['nodes'] if t.get('closer')]
          pr = closers[0] if closers else None
          if pr and 'associatedPullRequests' in pr:
              prs = pr['associatedPullRequests']['nodes']
              pr = prs[0] if prs else None
          if not pr or not pr.get('merged'): continue
          out.append({'issue': n['number'], 'title': n['title'], 'closed': n['closedAt'][:10],
                      'labels': [l['name'] for l in n['labels']['nodes']],
                      'body': (n['body'] or '')[:2500],
                      'pr': pr['number'], 'pr_title': pr['title'], 'pr_body': (pr['body'] or '')[:2000],
                      'pr_files': [f['path'] for f in pr['files']['nodes']]})
      print(len(out), s['issueCount'], file=sys.stderr)
      if not s['pageInfo']['hasNextPage']: break
      after = s['pageInfo']['endCursor']
  end = start - datetime.timedelta(days=1)
json.dump(out[:1000], open('issues.json', 'w'))
