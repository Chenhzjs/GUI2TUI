import unittest

from real_sites import check


class RealSiteOracleTests(unittest.TestCase):
    def test_scope_and_all_conditions_are_required(self):
        observed = {'errors': [], 'nodes': [
            {'id': 0, 'parent': None, 'role': 'document web', 'name': 'Article',
             'document': ['DocURL:https://example.org/article']},
            {'id': 1, 'parent': 0, 'role': 'radio button', 'name': 'Dark', 'states': []},
            {'id': 2, 'parent': None, 'role': 'radio button', 'name': 'Dark', 'states': ['checked']},
        ]}
        expected = {'url': 'https://example.org/article', 'nodes': [
            {'role': 'radio button', 'name': 'Dark', 'states': ['checked'], 'count': 1}]}
        self.assertFalse(check(expected, observed, {})['passed'])
        observed['nodes'][1]['states'] = ['checked']
        self.assertTrue(check(expected, observed, {})['passed'])
        expected['nodes'].append({'role': 'entry', 'name': 'Missing'})
        self.assertFalse(check(expected, observed, {})['passed'])

    def test_full_text_preview_cannot_pass_as_complete(self):
        observed = {'errors': [], 'nodes': [
            {'id': 0, 'parent': None, 'role': 'document web', 'name': 'Article',
             'document': ['DocURL:https://example.org/article']},
            {'id': 1, 'parent': 0, 'role': 'paragraph', 'text': 'A complete paragraph.'},
        ]}
        snapshot = {'nodes': [
            {'id': 10, 'parent': None, 'role': 'Document', 'label': 'Article'},
            {'id': 11, 'parent': 10, 'role': 'Paragraph', 'value': 'A complete…', 'partial': False},
        ]}
        expected = {'url': 'https://example.org/article', 'mirror': {
            'oracle': {'role': 'paragraph'}, 'g2t': {'role': 'Paragraph'}}}
        self.assertFalse(check(expected, observed, snapshot)['passed'])
        snapshot['nodes'][1]['value'] = 'A complete paragraph.'
        self.assertTrue(check(expected, observed, snapshot)['passed'])

    def test_wrong_fragment_or_duplicate_document_does_not_pass(self):
        doc = {'id': 0, 'parent': None, 'role': 'document web', 'name': 'Article',
               'document': ['DocURL:https://example.org/article#wrong']}
        observed = {'errors': [], 'nodes': [doc]}
        expected = {'url': 'https://example.org/article#right'}
        self.assertFalse(check(expected, observed, {})['passed'])
        doc['document'] = ['DocURL:https://example.org/article#right']
        observed['nodes'].append({**doc, 'id': 1})
        self.assertFalse(check(expected, observed, {})['passed'])

    def test_independent_content_alone_cannot_prove_g2t_export(self):
        observed = {'errors': [], 'nodes': [
            {'id': 0, 'parent': None, 'role': 'document web', 'name': 'Article',
             'document': ['DocURL:https://example.org/article']},
            {'id': 1, 'parent': 0, 'role': 'heading', 'name': 'Contents'},
        ]}
        expected = {'url': 'https://example.org/article',
                    'nodes': [{'role': 'heading', 'name': 'Contents'}],
                    'g2t_nodes': [{'role': 'Heading', 'label': 'Contents'}]}
        self.assertFalse(check(expected, observed, {'nodes': []})['passed'])
        snapshot = {'nodes': [
            {'id': 10, 'parent': None, 'role': 'Document', 'label': 'Article'},
            {'id': 11, 'parent': 10, 'role': 'Heading', 'label': 'Contents'},
        ]}
        self.assertTrue(check(expected, observed, snapshot)['passed'])


if __name__ == '__main__':
    unittest.main()
