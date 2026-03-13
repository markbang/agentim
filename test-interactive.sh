#!/bin/bash

# Test script for AgentIM interactive mode
# This simulates user input to test the full message pipeline

(
    # Register Claude agent
    echo "1"
    sleep 0.5
    echo "claude-test"
    sleep 0.5
    echo "1"
    sleep 0.5
    echo ""
    sleep 0.5

    # Register Telegram channel
    echo "2"
    sleep 0.5
    echo "telegram-test"
    sleep 0.5
    echo "1"
    sleep 0.5

    # Create session
    echo "3"
    sleep 0.5
    echo "1"
    sleep 0.5
    echo "1"
    sleep 0.5
    echo "testuser"
    sleep 0.5

    # Send message
    echo "4"
    sleep 0.5
    echo "1"
    sleep 0.5
    echo "Hello, what is 2+2?"
    sleep 1

    # View status
    echo "5"
    sleep 1

    # Exit
    echo "6"
) | ./target/release/agentim interactive
