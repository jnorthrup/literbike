#!/usr/bin/env python3

import argparse
import os
import json
import re

def analyze_file(filepath):
    """Analyzes a single file for lines of code and markers."""
    loc = 0
    markers = []
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            for i, line in enumerate(f, 1):
                line = line.strip()
                if line:
                    loc += 1
                if re.search(r'(TODO|FIXME|XXX)', line, re.IGNORECASE):
                    markers.append(i)
    except (IOError, UnicodeDecodeError):
        # Handle potential file read errors (e.g., binary files)
        return 0, []
    return loc, markers

def walk_directory(root_dir, exclude_dirs):
    """Recursively walks a directory, excluding specified directories."""
    files_scanned = 0
    total_loc = 0
    all_markers = {}  # {filepath: [line_numbers]}
    for dirpath, dirnames, filenames in os.walk(root_dir):
        # Exclude directories
        dirnames[:] = [d for d in dirnames if d not in exclude_dirs and not d.startswith('.')]
        for filename in filenames:
            filepath = os.path.join(dirpath, filename)
            if os.path.isfile(filepath):
                files_scanned += 1
                loc, markers = analyze_file(filepath)
                total_loc += loc
                if markers:
                    all_markers[filepath] = markers
    return files_scanned, total_loc, all_markers

def main():
    parser = argparse.ArgumentParser(description='Gap analysis for a codebase.')
    parser.add_argument('--root', default='.', help='Repository root directory')
    args = parser.parse_args()

    root_dir = os.path.abspath(args.root)
    exclude_dirs = ['.git', '.cargo', '.claude', 'target', 'node_modules']

    files_scanned, total_loc, all_markers = walk_directory(root_dir, exclude_dirs)

    total_markers = sum(len(markers) for markers in all_markers.values())

    # Get top 5 files with most markers
    sorted_markers = sorted(
        [(path, len(markers)) for path, markers in all_markers.items()],
        key=lambda item: item[1],
        reverse=True
    )
    top_marker_files = [{"path": path, "count": count} for path, count in sorted_markers[:5]]

    report = {
        "root": root_dir,
        "files_scanned": files_scanned,
        "total_loc": total_loc,
        "total_markers": total_markers,
        "top_marker_files": top_marker_files
    }

    print(json.dumps(report, indent=2))

if __name__ == "__main__":
    main()