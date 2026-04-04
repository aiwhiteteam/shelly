#!/bin/bash
set -e

cd "$(dirname "$0")/.."

echo "Installing dependencies..."
npm install

echo "Building frontend..."
npm run build:frontend

echo "Starting Shelly..."
cargo tauri dev
