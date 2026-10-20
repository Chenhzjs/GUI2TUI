import unittest

from revised import asserted_v2, resolve_plan


class RevisedValidationTests(unittest.TestCase):
    def test_duplicate_labels_require_structural_evidence(self):
        observation = {'nodes': [
            {'id': 1, 'parent': None, 'label': 'Delivery speed', 'role': 'ComboBox', 'states': []},
            {'id': 2, 'parent': 1, 'label': 'Express delivery', 'role': 'MenuItem', 'states': []},
            {'id': 3, 'parent': None, 'label': 'Express delivery', 'role': 'MenuItem', 'states': []},
        ], 'operations': [
            {'id': 'a', 'target': 2, 'kind': 'selection', 'selected': True},
            {'id': 'b', 'target': 3, 'kind': 'selection', 'selected': True},
        ]}
        plan = {'label': 'Express delivery', 'kind': 'selection', 'selected': True}
        with self.assertRaises(LookupError):
            resolve_plan(observation, plan)
        chosen = resolve_plan(observation, {**plan, 'ancestor': {'role': 'ComboBox', 'label': 'Delivery speed'}})
        self.assertEqual(chosen['operation_id'], 'a')

    def test_matching_text_cannot_override_do_not_submit(self):
        task = {'expect_node': {'name': 'Full name', 'value': 'literal'}, 'forbid_events': ['profile']}
        observed = {'nodes': [{'name': 'Full name', 'text': 'literal'}]}
        self.assertTrue(asserted_v2(task, [], observed))
        self.assertFalse(asserted_v2(task, [{'name': 'profile', 'value': {}}], observed))

    def test_unicode_oracle_requires_independent_fresh_evidence(self):
        task = {'expect_public_text_literal': {'label': 'Full name', 'value': '🧪'},
                'forbid_events': ['profile']}
        evidence = {'label': 'Full name', 'literal': '🧪', 'verified': False}
        observed = {'public_text_evidence': evidence}
        self.assertFalse(asserted_v2(task, [], observed))
        evidence['verified'] = True
        self.assertTrue(asserted_v2(task, [], observed))
        evidence['literal'] += '\ufeff'
        self.assertFalse(asserted_v2(task, [], observed))


if __name__ == '__main__':
    unittest.main()
