#!/usr/bin/env bash

for i in {1..100000}; do
  echo "FALSE"      # Column 1: Checkbox status
  echo "Item $i"    # Column 2: The Name
  echo "Category $i" # Column 3: Category
  echo "Info $i"    # Column 4: Info
  echo "Detail $i"  # Column 5: Detail
  echo "Note $i"    # Column 6: Note
  echo "Tag $i"     # Column 7: Tag
  echo "Extra $i"   # Column 8: Extra
done | zenity-rs --list --checklist \
  --title="Large Checklist Test" \
  --column="Check" --column="Item Name" --column="Category" --column="Info" --column="Detail" --column="Note" --column="Tag" --column="Extra" \
  --width=500 --height=600
