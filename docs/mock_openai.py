#!/usr/bin/env python3
"""本地模拟 OpenAI 兼容 /v1/chat/completions 端点，用于测试（见 test-cases.md）

GET /set-fail/1 或 /set-fail/0 可切换总结请求是否返回 500（TC-36）。
"""
import json
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    fail_summary = False

    def do_GET(self):
        if self.path.startswith('/set-fail/'):
            Handler.fail_summary = self.path.rstrip('/').endswith('/1')
            self.send_response(200)
            self.send_header('Content-Length', '2')
            self.end_headers()
            self.wfile.write(b'ok')
            return
        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0))
        body = json.loads(self.rfile.read(n))
        system = body['messages'][0]['content']
        low = system.lower()
        if '出题' in system or 'question' in low:
            reply = {
                "topic": "TCP 三次握手",
                "difficulty": "medium",
                "question": "请描述TCP三次握手的过程",
                "reference_answer": "SYN -> SYN-ACK -> ACK",
            }
        elif '评判' in system or 'judge' in low:
            reply = {
                "score": 75,
                "feedback": "整体正确，细节不足",
                "topic_tags": ["TCP", "三次握手"],
            }
        else:  # 总结
            if Handler.fail_summary:
                self.send_response(500)
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(b'{"error":"mock summary failure"}')
                return
            reply = {
                "weak_topics": ["TCP"],
                "related_weaknesses": [],
                "suggestions": ["多练习传输层"],
            }
        content = json.dumps(reply, ensure_ascii=False)
        data = json.dumps(
            {"choices": [{"message": {"content": content}}]},
            ensure_ascii=False,
        ).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


if __name__ == '__main__':
    HTTPServer(('127.0.0.1', 8765), Handler).serve_forever()
