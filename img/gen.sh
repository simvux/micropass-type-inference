#!/bin/env zsh

input=${@: -1}

# Showcase
# freeze --theme dracula --font.family "JetBrainsMono Nerd Font" --padding "40,240,40,20" -o "$input.png" "$input" "${@[1,-2]}"

# Feature Complete Example
freeze --theme dracula --font.family "JetBrainsMono Nerd Font" --padding "70,240,70,20" -o "$input.png" "$input" "${@[1,-2]}"

# padding: top,right,bottom,left
