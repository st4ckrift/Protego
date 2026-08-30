import sys
import os
import json
from pathlib import Path

# Ensure project root is on sys.path
BASE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE_DIR))

from gate.database import init_db
from agent.agent_service import process_agent_request

def main():
    init_db()
    
    if len(sys.argv) > 1:
        prompt = " ".join(sys.argv[1:])
        print(f"\n🤖 Processing prompt: '{prompt}'\n" + "="*50)
        res = process_agent_request(prompt)
        print(res["agent_message"])
        print("\n--- Gate Decision Payload ---")
        print(json.dumps(res["gate_decision"], indent=2))
        return

    print("==================================================")
    print(" 🛡️  Agentic Commerce Spend-Gate CLI Simulator  🛡️")
    print("==================================================")
    print("Type your shopping request (e.g., 'Order ₹500 groceries from Zepto for user_rahul')")
    print("Type 'exit' or 'quit' to exit.\n")

    while True:
        try:
            prompt = input("Agent Prompt > ").strip()
            if not prompt:
                continue
            if prompt.lower() in ["exit", "quit", "q"]:
                print("Exiting CLI simulator.")
                break
            
            res = process_agent_request(prompt)
            print("\n" + res["agent_message"])
            print("\n[Gate Raw Decision]:", json.dumps(res["gate_decision"]))
            print("-" * 50 + "\n")
        except (KeyboardInterrupt, EOFError):
            print("\nExiting.")
            break

if __name__ == "__main__":
    main()
