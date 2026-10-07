"""Regression checks: the task harness must never replace GUI2TUI actuation."""
import ast
import json
from pathlib import Path
import unittest
from scenario_session import SCENARIOS

HERE = Path(__file__).parent


class TaskContract(unittest.TestCase):
    def test_no_direct_gui_delivery(self):
        forbidden = {'queryEditableText', 'doAction', 'setTextContents',
                     'insertText', 'deleteText', 'grabFocus', 'generateKeyboardEvent',
                     'generateMouseEvent', 'selectChild', 'deselectChild', 'clearSelection'}
        for script in ('scenario_session.py', 'public_operations_session.py'):
            tree = ast.parse((HERE / script).read_text())
            for node in ast.walk(tree):
                if isinstance(node, ast.Attribute):
                    self.assertNotIn(node.attr, forbidden)

    def test_manifest_tasks_are_executable_and_unique(self):
        rows = json.loads((HERE / 'task_scenarios.json').read_text())['scenarios']
        pairs = [(r['application'], r['scenario']) for r in rows]
        self.assertEqual(len(pairs), len(set(pairs)))
        self.assertEqual(set(pairs), {(a,s) for a, tasks in SCENARIOS.items() for s in tasks})
        self.assertTrue(all(len(tasks) >= 5 for tasks in SCENARIOS.values()))


if __name__ == '__main__':
    unittest.main()
