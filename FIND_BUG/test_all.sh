#!/bin/bash
# Test all FIND_BUG .lz files
# Output: file, stage (lz/success), result (ok/error), details

LZC="cargo run --quiet --"
RESULTS="FIND_BUG/test_results.txt"
> "$RESULTS"

for f in $(find FIND_BUG -name "*.lz" | sort); do
    echo -n "Testing: $f ... "
    
    # Run lz compiler
    OUTPUT=$($LZC "$f" 2>&1)
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -ne 0 ]; then
        # lz compiler error (parse or other)
        ERROR_MSG=$(echo "$OUTPUT" | head -1)
        echo "LZ_ERROR: $ERROR_MSG" | tee -a "$RESULTS"
        continue
    fi
    
    # lz compiled successfully - check if .rs was generated
    RS_FILE="${f%.lz}.rs"
    if [ ! -f "$RS_FILE" ]; then
        echo "LZ_ERROR: no .rs generated" | tee -a "$RESULTS"
        continue
    fi
    
    # Now try rustc
    BUILTINS="target/debug/liblz_builtins.rlib"
    if [ ! -f "$BUILTINS" ]; then
        # Build the builtins library
        cargo build --package lz_builtins 2>/dev/null
    fi
    
    RUSTC_OUTPUT=$(rustc --edition 2021 --extern lz_builtins="$BUILTINS" -A warnings "$RS_FILE" -o /dev/null 2>&1)
    RUSTC_EXIT=$?
    
    if [ $RUSTC_EXIT -eq 0 ]; then
        echo "OK" | tee -a "$RESULTS"
    else
        # Extract first error
        FIRST_ERROR=$(echo "$RUSTC_OUTPUT" | grep "^error" | head -1)
        echo "RUSTC_ERROR: $FIRST_ERROR" | tee -a "$RESULTS"
    fi
done

echo ""
echo "=== SUMMARY ==="
echo "Total files: $(wc -l < <(find FIND_BUG -name "*.lz"))"
echo "LZ errors: $(grep -c "LZ_ERROR" "$RESULTS" 2>/dev/null || echo 0)"
echo "Rustc errors: $(grep -c "RUSTC_ERROR" "$RESULTS" 2>/dev/null || echo 0)"
echo "OK: $(grep -c "^OK$" "$RESULTS" 2>/dev/null || echo 0)"
