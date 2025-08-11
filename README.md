# CSV Directory Concatenator

A high-performance Rust utility that concatenates CSV files from subdirectories into consolidated output files, handling header normalization and column reordering.

## Overview

This tool processes a directory structure containing subdirectories with CSV files, combining all CSV files within each subdirectory into a single output CSV file.

## Features

- **Automatic CSV discovery**: Scans subdirectories for CSV files
- **Header normalization**: Handles different column orders and missing/extra columns
- **BOM handling**: Automatically strips UTF-8 BOM from CSV headers
- **Flexible delimiters**: Supports custom delimiter characters
- **Atomic file operations**: Uses temporary files to ensure data integrity
- **Robust error handling**: Detailed error messages with context
- **Deterministic output**: Stable sorting for reproducible results

## Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)

### Building from Source

```bash
# Clone the repository
git clone <repository-url>
cd concat_files

# Build the project
cargo build --release

# The binary will be available at ./target/release/csv_per_dir_cat
```

## Usage

```bash
csv_per_dir_cat [root_dir] [output_dir] [delimiter]
```

### Parameters

- `root_dir` (optional): The root directory to scan for subdirectories containing CSV files. Defaults to current directory (`.`)
- `output_dir` (optional): Directory where concatenated CSV files will be saved. Defaults to `./_out`
- `delimiter` (optional): Single ASCII character to use as CSV delimiter. Defaults to comma (`,`)

### Examples

```bash
# Use default settings (current directory, output to ./_out, comma delimiter)
csv_per_dir_cat

# Specify custom directories
csv_per_dir_cat /path/to/data /path/to/output

# Use semicolon as delimiter
csv_per_dir_cat /path/to/data /path/to/output ";"

# Process current directory with tab delimiter
csv_per_dir_cat . ./_out $'\t'
```

## How It Works

1. **Directory Scanning**: The tool scans the root directory for immediate subdirectories
2. **CSV Collection**: For each subdirectory, it collects all `.csv` files (non-recursive)
3. **Header Analysis**: Takes the first CSV file's header as the canonical column structure
4. **Column Mapping**: For each subsequent file:
   - Maps columns to match the canonical header order
   - Fills missing columns with empty values
   - Ignores extra columns not in the canonical header
5. **Output Generation**: Creates a consolidated CSV file named `{subdirectory_name}.csv` in the output directory

## Data Processing Rules

### Header Handling
- The first CSV file in each subdirectory determines the canonical header structure
- Headers are compared case-sensitively
- UTF-8 BOM is automatically stripped if present

### Column Management
- **Missing columns**: Filled with empty strings in the output
- **Extra columns**: Ignored (with warning)
- **Different order**: Automatically reordered to match canonical structure

### File Processing Order
- Subdirectories are processed in alphabetical order
- CSV files within each subdirectory are processed in alphabetical order

## Example Structure

Given this directory structure:
```
data/
├── payments/
│   ├── january.csv
│   └── february.csv
├── refunds/
│   ├── refund_01.csv
│   └── refund_02.csv
└── reports/
    └── summary.csv
```

Running `csv_per_dir_cat data output` will produce:
```
output/
├── payments.csv    # Contains combined data from january.csv and february.csv
├── refunds.csv     # Contains combined data from refund_01.csv and refund_02.csv
└── reports.csv     # Contains data from summary.csv
```

## Error Handling

The tool provides detailed error messages including:
- File paths where errors occurred
- Context about the operation being performed
- Warnings for non-critical issues (e.g., column mismatches)

## Performance Considerations

- Uses buffered I/O for efficient file reading
- Processes files sequentially within each directory
- Memory usage scales with the size of individual CSV files, not the total dataset

## Dependencies

- `anyhow` (1.0): Error handling with context
- `csv` (1.3): CSV parsing and writing

## License

This project is distributed under the license specified in the LICENSE file.

## Contributing

Contributions are welcome! Please ensure your code:
- Follows Rust best practices
- Includes appropriate error handling
- Maintains backward compatibility
- Includes tests for new functionality

## Troubleshooting

### Common Issues

1. **"Delimiter must be a single ASCII character"**: Ensure you're providing a single character as the delimiter
2. **"No subdirectories under..."**: The tool only processes immediate subdirectories, not the root directory itself
3. **Header mismatch warnings**: These are informational - the tool will handle the differences automatically

### Debug Output

The tool provides informational output about:
- Directories being processed
- Files being skipped (no CSV files)
- Header mismatches and column differences
- Output file locations