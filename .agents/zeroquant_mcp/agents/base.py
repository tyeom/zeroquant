"""Base agent class"""

import subprocess
from pathlib import Path
from typing import Any
import logging

logger = logging.getLogger(__name__)


class BaseAgent:
    """모든 Agent의 기본 클래스"""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.logger = logger.getChild(self.__class__.__name__)

    async def execute(self, arguments: dict[str, Any]) -> str:
        """에이전트 실행 (하위 클래스에서 구현)"""
        raise NotImplementedError

    def run_command(
        self,
        cmd: list[str],
        cwd: Path | None = None,
        timeout: int = 300
    ) -> tuple[int, str, str]:
        """명령어 실행

        Returns:
            (return_code, stdout, stderr)
        """
        if cwd is None:
            cwd = self.project_root

        self.logger.info(f"Running: {' '.join(cmd)} in {cwd}")

        try:
            result = subprocess.run(
                cmd,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout,
                encoding='utf-8',
                errors='replace'
            )
            return result.returncode, result.stdout, result.stderr

        except subprocess.TimeoutExpired:
            return -1, "", f"Command timed out after {timeout}s"
        except Exception as e:
            return -1, "", str(e)

    def format_success(self, title: str, content: str) -> str:
        """성공 메시지 포맷"""
        return f"✅ {title}\n\n{content}"

    def format_error(self, title: str, content: str) -> str:
        """에러 메시지 포맷"""
        return f"❌ {title}\n\n{content}"

    def format_warning(self, title: str, content: str) -> str:
        """경고 메시지 포맷"""
        return f"⚠️ {title}\n\n{content}"

    def format_section(self, title: str, content: str) -> str:
        """섹션 포맷"""
        return f"## {title}\n\n{content}\n"
