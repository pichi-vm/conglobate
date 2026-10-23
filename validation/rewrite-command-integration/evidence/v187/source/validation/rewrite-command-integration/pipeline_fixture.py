"""Committed pass-through CLI inputs; no host privileged operations."""
import pathlib
import subprocess


class PipelineFixture:
    def __init__(self, root):
        self.root = root

    def create(self):
        repo = self.root / 'pipeline-context'
        repo.mkdir()
        (repo / 'recipes').mkdir()
        recipe = 'carapace:\n  from: example.test/base:1\n'
        (repo / 'recipes/image.yml').write_text(recipe)
        (repo / 'recipes/unsupported.yml').write_text(recipe + '  derive:\n    - run: echo MUST_NOT_RUN\n')
        pin = (self.root / 'cache-fixture/chain.pin').read_text().strip()
        (repo / 'images.lock').write_text(f'version: 1\nimages:\n  example.test/base:1:\n    index: {pin}\n')
        subprocess.run(['git', 'init', '--quiet', str(repo)], check=True)
        subprocess.run(['git', '-C', str(repo), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(repo), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        '-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'Pipeline fixture'], check=True)
        (self.root / 'pipeline-commit').write_text(subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True))
        (self.root / 'pipeline-recipe-committed').write_text(recipe)
        # The production binary must read the requested commit, not this worktree.
        (repo / 'recipes/image.yml').write_text('not the committed recipe\n')
