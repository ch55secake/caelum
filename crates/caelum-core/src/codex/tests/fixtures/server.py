
import json, os, sys
models = {'thread-existing': 'saved-model'}
efforts = {'thread-existing': 'low'}
archived = set()
deleted = set()
for raw in sys.stdin:
    request = json.loads(raw)
    method = request.get('method')
    request_id = request.get('id')
    params = request.get('params', {})
    if method is None and request_id == 'server-approval-1':
        print(json.dumps({'method': 'test/approvalResolved', 'params': {'decision': request['result']['decision']}}), flush=True)
        continue
    if method is None and request_id in [41, '41']:
        print(json.dumps({'method': 'test/questionResolved', 'params': request}), flush=True)
        continue
    if request_id is None:
        continue
    if method == 'initialize':
        assert params['capabilities']['experimentalApi'] is True
        result = {'userAgent': 'fake-codex'}
    elif method == 'thread/list':
        present = ('thread-existing' in archived) == params['archived'] and 'thread-existing' not in deleted
        result = {'data': [{'id': 'thread-existing', 'name': 'Existing', 'preview': 'Earlier work', 'updatedAt': 123, 'status': {'type': 'active' if os.path.exists('active-thread') else 'idle'}}] if present else [], 'nextCursor': None}
    elif method == 'thread/read':
        result = {'thread': {'id': params['threadId'], 'cwd': os.path.dirname(os.getcwd()) if params['threadId'] == 'foreign-thread' else os.getcwd(), 'gitInfo': {'branch': 'feature/sidebar'}, 'status': {'type': 'active' if os.path.exists('active-thread') else 'idle'}}}
    elif method in ['thread/archive', 'thread/unarchive']:
        if os.path.exists('archive-error'):
            print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'archive unavailable'}}), flush=True)
            continue
        if method == 'thread/archive': archived.add(params['threadId'])
        else: archived.discard(params['threadId'])
        result = {}
    elif method == 'thread/delete':
        if os.path.exists('delete-error'):
            print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'delete unavailable'}}), flush=True)
            continue
        deleted.add(params['threadId'])
        result = {}
    elif method == 'thread/start':
        models['thread-new'] = params.get('model') or 'default-model'
        efforts['thread-new'] = 'medium'
        result = {'thread': {'id': 'thread-new', 'preview': '', 'name': None}, 'model': models['thread-new'], 'reasoningEffort': efforts['thread-new']}
    elif method == 'thread/resume':
        result = {'thread': {'id': params['threadId'], 'name': 'Resumed', 'preview': 'History'}, 'model': models[params['threadId']], 'reasoningEffort': efforts[params['threadId']]}
    elif method == 'model/list':
        result = {'data': [{'model': 'test-model', 'displayName': 'Test model', 'isDefault': True, 'defaultReasoningEffort': 'medium', 'supportedReasoningEfforts': [{'reasoningEffort': 'low'}, {'reasoningEffort': 'medium'}, {'reasoningEffort': 'high'}]}], 'nextCursor': None}
    elif method == 'config/read':
        assert params['cwd'] == os.getcwd()
        result = {'config': {'model': 'configured-model'}}
    elif method == 'thread/turns/list':
        assert params['itemsView'] == 'full' and params['sortDirection'] == 'desc'
        if params['limit'] == 1:
            if os.path.exists('missing-details'):
                print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'no history yet'}}), flush=True)
                continue
            result = {'data': [{'id': 'latest', 'items': [
                {'type': 'fileChange', 'status': 'completed', 'changes': [
                    {'kind': {'type': 'update'}, 'diff': '--- a/file\n+++ b/file\n@@ -1 +1,2 @@\n-old\n+new\n+extra\n'},
                    {'kind': {'type': 'add'}, 'diff': 'first\nsecond\n'}]},
                {'type': 'fileChange', 'status': 'failed', 'changes': [{'kind': {'type': 'add'}, 'diff': 'not applied'}]}]}], 'nextCursor': 'older'}
        elif params.get('cursor'):
            assert params['cursor'] == 'older-cursor'
            result = {'data': [{'id': 'turn-old', 'items': [{'id': 'old-user', 'type': 'userMessage', 'content': [{'type': 'text', 'text': 'Oldest'}]}]}], 'nextCursor': None}
        else:
            assert params['limit'] == 30
            result = {'data': [{'id': 'turn-b', 'items': [{'id': 'cmd', 'type': 'commandExecution', 'command': 'git status', 'aggregatedOutput': 'clean', 'status': 'completed'}]}, {'id': 'turn-a', 'items': []}], 'nextCursor': 'older-cursor'}
    elif method == 'turn/start':
        with open('turn-request.json', 'w') as request:
            json.dump(params, request)
        prompt = params['input'][0]['text']
        if prompt == 'rpc-error':
            print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'fake request error'}}), flush=True)
            continue
        if params.get('model'):
            models[params['threadId']] = params['model']
        if params.get('effort'):
            efforts[params['threadId']] = params['effort']
        if prompt == 'request-approval':
            print(json.dumps({'id': 'server-approval-1', 'method': 'item/commandExecution/requestApproval', 'params': {'threadId': params['threadId'], 'turnId': 'turn-approval', 'itemId': 'command-1', 'command': ['git', 'status'], 'cwd': '/project', 'availableDecisions': ['accept', 'decline']}}), flush=True)
        if prompt == 'request-question':
            for question_id in [41, '41']:
                print(json.dumps({'id': question_id, 'method': 'item/tool/requestUserInput', 'params': {'threadId': params['threadId'], 'turnId': 'turn-question', 'itemId': 'question-tool', 'questions': [
                    {'id': 'choice', 'header': 'Approach', 'question': 'Which approach?', 'options': [{'label': 'Small', 'description': 'Focused change'}, {'label': 'Large', 'description': 'Full rewrite'}]},
                    {'id': 'detail', 'header': 'Detail', 'question': 'Any details?', 'isOther': True, 'isSecret': True, 'options': None}
                ]}}), flush=True)
            print(json.dumps({'id': request_id, 'result': {'turn': {'id': 'turn-question', 'status': 'inProgress'}}}), flush=True)
            continue
        if prompt == 'resolve-question':
            for question_id in [41, '41']:
                print(json.dumps({'method': 'serverRequest/resolved', 'params': {'threadId': params['threadId'], 'requestId': question_id}}), flush=True)
        print(json.dumps({'method': 'turn/started', 'params': {'turn': {'id': 'turn-1'}}}), flush=True)
        print(json.dumps({'method': 'item/agentMessage/delta', 'params': {'threadId': params['threadId'], 'itemId': 'item-1', 'delta': 'Hello from Codex'}}), flush=True)
        print(json.dumps({'method': 'turn/completed', 'params': {'turn': {'id': 'turn-1', 'status': 'completed'}}}), flush=True)
        result = {'turn': {'id': 'turn-1', 'status': 'inProgress'}}
    elif method == 'turn/interrupt':
        result = {}
    else:
        print(json.dumps({'id': request_id, 'error': {'code': -32601, 'message': 'Unknown method'}}), flush=True)
        continue
    print(json.dumps({'id': request_id, 'result': result}), flush=True)
    if method == 'thread/list' and os.path.exists('die-after-list'):
        os.unlink('die-after-list')
        sys.exit(0)
