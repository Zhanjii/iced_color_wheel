# -*- mode: python ; coding: utf-8 -*-
"""
PyInstaller spec file for building standalone executables.

Usage:
    pyinstaller app_name.spec --clean

For one-file build (slower startup, but single file):
    pyinstaller app_name.spec --clean --onefile

Build variants controlled by environment:
    DEV_BUILD=1 pyinstaller app_name.spec  # Include console for debugging

This spec file handles:
- Platform-specific binaries and hidden imports
- Icon embedding (Windows/macOS)
- Resource bundling (credentials, configs, icons)
- Version info (Windows)
- macOS app bundle creation

Customize:
1. Update APP_NAME and paths for your project
2. Add your hidden imports
3. Configure data files to bundle
4. Set up code signing (production builds)
"""

import os
import sys
from pathlib import Path

# =============================================================================
# CONFIGURATION
# =============================================================================

# Project paths
SPECPATH_DIR = Path(SPECPATH)  # Directory containing this spec file
PROJECT_ROOT = SPECPATH_DIR.parent if SPECPATH_DIR.name == "build" else SPECPATH_DIR
SRC_DIR = PROJECT_ROOT / "src"

# Application metadata
APP_NAME = "YourAppName"
APP_BUNDLE_ID = "com.yourcompany.yourappname"

# Read version from package
try:
    init_file = SRC_DIR / "app_name" / "__init__.py"
    for line in init_file.read_text().splitlines():
        if line.startswith("__version__"):
            APP_VERSION = line.split('"')[1]
            break
    else:
        APP_VERSION = "0.0.0"
except Exception:
    APP_VERSION = "0.0.0"

# Platform detection
IS_WINDOWS = sys.platform.startswith("win")
IS_MACOS = sys.platform == "darwin"
IS_LINUX = sys.platform.startswith("linux")

# Build mode
IS_DEV_BUILD = os.environ.get("DEV_BUILD", "0") == "1"

# =============================================================================
# ANALYSIS
# =============================================================================

block_cipher = None

# Entry point
entry_point = str(SRC_DIR / "app_name" / "main.py")

# Hidden imports that PyInstaller misses
hidden_imports = [
    # CustomTkinter
    "customtkinter",
    "PIL._tkinter_finder",

    # Keyring backends
    "keyring.backends.Windows" if IS_WINDOWS else None,
    "keyring.backends.macOS" if IS_MACOS else None,
    "keyring.backends.SecretService" if IS_LINUX else None,

    # Pystray backends
    "pystray._win32" if IS_WINDOWS else None,
    "pystray._darwin" if IS_MACOS else None,
    "pystray._xorg" if IS_LINUX else None,

    # Cryptography
    "cryptography.fernet",

    # Add your hidden imports here
    # "your_package.module",
]

# Remove None entries
hidden_imports = [h for h in hidden_imports if h]

# Data files to bundle
# Format: (source_path, destination_folder_in_bundle)
data_files = [
    # Icons
    (str(SRC_DIR / "app_name" / "ui" / "icon"), "app_name/ui/icon"),

    # Bundled credentials (encrypted)
    (str(PROJECT_ROOT / "keys"), "keys"),

    # Any other resources
    # (str(PROJECT_ROOT / "resources"), "resources"),
]

# Filter out non-existent paths
data_files = [(src, dst) for src, dst in data_files if Path(src).exists()]

# Modules to exclude (reduce bundle size)
excludes = [
    "matplotlib",
    "numpy",
    "pandas",
    "scipy",
    "IPython",
    "jupyter",
    "notebook",
    "test",
    "tests",
    "unittest",
]

# =============================================================================
# ANALYSIS OBJECT
# =============================================================================

a = Analysis(
    [entry_point],
    pathex=[str(SRC_DIR)],
    binaries=[],
    datas=data_files,
    hiddenimports=hidden_imports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=excludes,
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

# =============================================================================
# PYZ ARCHIVE
# =============================================================================

pyz = PYZ(
    a.pure,
    a.zipped_data,
    cipher=block_cipher,
)

# =============================================================================
# EXECUTABLE
# =============================================================================

# Icon paths
if IS_WINDOWS:
    icon_path = str(SRC_DIR / "app_name" / "ui" / "icon" / "app.ico")
    version_file = str(PROJECT_ROOT / "version_info.txt")
elif IS_MACOS:
    icon_path = str(SRC_DIR / "app_name" / "ui" / "icon" / "app.icns")
    version_file = None
else:
    icon_path = None
    version_file = None

# Check if icon exists
if icon_path and not Path(icon_path).exists():
    icon_path = None

# Check if version file exists
if version_file and not Path(version_file).exists():
    version_file = None

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name=APP_NAME,
    debug=IS_DEV_BUILD,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,  # Compress executable
    upx_exclude=[],
    runtime_tmpdir=None,
    console=IS_DEV_BUILD,  # Show console in dev builds
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,

    # Windows-specific
    icon=icon_path if IS_WINDOWS else None,
    version=version_file,

    # macOS-specific
    # icon is set in BUNDLE below
)

# =============================================================================
# macOS APP BUNDLE
# =============================================================================

if IS_MACOS:
    app = BUNDLE(
        exe,
        name=f"{APP_NAME}.app",
        icon=icon_path,
        bundle_identifier=APP_BUNDLE_ID,
        info_plist={
            "CFBundleName": APP_NAME,
            "CFBundleDisplayName": APP_NAME,
            "CFBundleVersion": APP_VERSION,
            "CFBundleShortVersionString": APP_VERSION,
            "NSHighResolutionCapable": True,
            "LSMinimumSystemVersion": "10.13",
            "NSRequiresAquaSystemAppearance": False,  # Support dark mode
        },
    )

# =============================================================================
# WINDOWS FOLDER BUILD (Alternative to single file)
# =============================================================================

# Uncomment for folder-based distribution (faster startup)
# coll = COLLECT(
#     exe,
#     a.binaries,
#     a.zipfiles,
#     a.datas,
#     strip=False,
#     upx=True,
#     upx_exclude=[],
#     name=APP_NAME,
# )
