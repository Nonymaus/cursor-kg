#!/usr/bin/env python3
"""Test Cursor-style MCP connection to the SSE server"""

import asyncio
import aiohttp
import json
import uuid

async def test_cursor_connection():
    """Test the same connection pattern that Cursor uses"""
    
    print("🔌 Testing Cursor-style MCP connection...")
    
    try:
        # Step 1: Connect to SSE endpoint
        print("1️⃣ Connecting to SSE endpoint...")
        async with aiohttp.ClientSession() as session:
            
            # Connect to SSE stream
            async with session.get('http://localhost:8360/sse') as sse_response:
                if sse_response.status == 200:
                    print("✅ SSE connection established")
                    
                    # Read the first few lines to get the session endpoint
                    content = await sse_response.content.readline()
                    if b'endpoint' in content:
                        endpoint_line = content.decode()
                        if '/messages/' in endpoint_line:
                            session_endpoint = endpoint_line.split('data: ')[1].strip()
                            print(f"✅ Session endpoint: {session_endpoint}")
                            
                            # Step 2: Initialize MCP session
                            print("\n2️⃣ Initializing MCP session...")
                            init_request = {
                                "jsonrpc": "2.0",
                                "id": 1,
                                "method": "initialize",
                                "params": {
                                    "protocolVersion": "2024-11-05",
                                    "capabilities": {
                                        "tools": {}
                                    },
                                    "clientInfo": {
                                        "name": "cursor-test",
                                        "version": "1.0.0"
                                    }
                                }
                            }
                            
                            full_url = f"http://localhost:8360{session_endpoint}"
                            async with session.post(full_url, json=init_request) as init_response:
                                if init_response.status in [200, 202]:
                                    print("✅ MCP session initialized")
                                    
                                    # Step 3: List tools
                                    print("\n3️⃣ Requesting tools list...")
                                    tools_request = {
                                        "jsonrpc": "2.0",
                                        "id": 2,
                                        "method": "tools/list",
                                        "params": {}
                                    }
                                    
                                    async with session.post(full_url, json=tools_request) as tools_response:
                                        if tools_response.status in [200, 202]:
                                            print("✅ Tools list request successful")
                                            
                                            # Try to read the response
                                            try:
                                                response_text = await tools_response.text()
                                                if response_text:
                                                    tools_data = json.loads(response_text)
                                                    if 'result' in tools_data and 'tools' in tools_data['result']:
                                                        tools = tools_data['result']['tools']
                                                        print(f"🎯 Successfully retrieved {len(tools)} tools!")
                                                        
                                                        for i, tool in enumerate(tools, 1):
                                                            name = tool.get('name', 'Unknown')
                                                            desc = tool.get('description', 'No description')
                                                            print(f"  {i:2d}. {name}")
                                                        
                                                        return True
                                                    else:
                                                        print(f"❌ Unexpected tools response: {tools_data}")
                                                else:
                                                    print("⚠️ Empty response from tools endpoint")
                                            except json.JSONDecodeError as e:
                                                print(f"❌ Invalid JSON response: {e}")
                                        else:
                                            print(f"❌ Tools request failed: {tools_response.status}")
                                else:
                                    print(f"❌ Session initialization failed: {init_response.status}")
                        else:
                            print("❌ No session endpoint found in SSE response")
                    else:
                        print("❌ Invalid SSE response format")
                else:
                    print(f"❌ SSE connection failed: {sse_response.status}")
                    
    except Exception as e:
        print(f"❌ Connection test failed: {e}")
        import traceback
        traceback.print_exc()
        return False
    
    return False

if __name__ == "__main__":
    success = asyncio.run(test_cursor_connection())
    if success:
        print("\n🎉 Connection test successful! Tools should be visible in Cursor.")
    else:
        print("\n💡 If tools still aren't visible in Cursor:")
        print("   1. Restart Cursor")
        print("   2. Check Cursor's MCP panel for any error messages")
        print("   3. Verify the .cursor/mcp.json configuration")
    
    print(f"\n📋 MCP Configuration reminder:")
    print(f"   File: .cursor/mcp.json")
    print(f"   URL: http://localhost:8360/sse")
    print(f"   Server: kg-mcp-server") 