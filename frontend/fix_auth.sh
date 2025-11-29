#!/bin/bash

# Script to fix cookie-based authentication in frontend files
# This removes Authorization headers and localStorage token usage

FILES=$(grep -r "\.header(\"Authorization\"" src/ --files-with-matches)

echo "Fixing authentication in frontend files..."
echo "Found $(echo "$FILES" | wc -l | xargs) files to update"
echo ""

for file in $FILES; do
    echo "Processing: $file"

    # Create backup
    cp "$file" "$file.bak"

    # Remove lines with .header("Authorization", ...)
    # Also remove the preceding line if it clones token
    sed -i '' '/let token = token\.clone();/d' "$file"
    sed -i '' '/\.header("Authorization", &format!("Bearer {}", token))/d' "$file"

    # Remove standalone token variable usage patterns
    sed -i '' '/if let Ok(Some(token)) = storage\.get_item("token")/,/^[[:space:]]*}[[:space:]]*$/d' "$file"
    sed -i '' '/if let Ok(Some(storage)) = window\.local_storage()/,/^[[:space:]]*}[[:space:]]*$/d' "$file"

    echo "  ✓ Removed Authorization headers"
done

echo ""
echo "Done! Backups saved as *.bak"
echo ""
echo "Next step: Add .credentials(web_sys::RequestCredentials::Include) to all Request calls"
echo "This needs manual review as placement matters for the builder pattern"
