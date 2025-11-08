"""
Setup script for the AniDB Client Python library.
"""

import platform
import sys
from pathlib import Path

from setuptools import find_packages, setup

# Read long description from README
readme_path = Path(__file__).parent / "README.md"
long_description = ""
if readme_path.exists():
    long_description = readme_path.read_text(encoding="utf-8")

# Platform-specific dependencies
install_requires = []
if platform.system() == "Windows":
    install_requires.append("pywin32>=300")

# Development dependencies
extras_require = {
    "dev": [
        "pytest>=7.0",
        "pytest-asyncio>=0.21",
        "pytest-cov>=4.0",
        "black>=23.0",
        "mypy>=1.0",
        "ruff>=0.1",
    ],
    "docs": [
        "sphinx>=6.0",
        "sphinx-rtd-theme>=1.3",
        "sphinx-autodoc-typehints>=1.23",
    ],
}

setup(
    name="anidb-client",
    version="0.1.0a1",
    author="Alysson Souza",
    author_email="",
    description="Python bindings for the AniDB Client Core Library",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/anidb-client",
    project_urls={
        "Bug Tracker": "https://github.com/yourusername/anidb-client/issues",
        "Documentation": "https://anidb-client.readthedocs.io",
        "Source Code": "https://github.com/yourusername/anidb-client",
    },
    license="MIT",
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: Multimedia :: Video",
        "Topic :: Software Development :: Libraries :: Python Modules",
        "Typing :: Typed",
    ],
    package_dir={"": "src"},
    packages=find_packages(where="src"),
    python_requires=">=3.8",
    install_requires=install_requires,
    extras_require=extras_require,
    package_data={
        "anidb_client": ["py.typed"],
    },
    zip_safe=False,
)