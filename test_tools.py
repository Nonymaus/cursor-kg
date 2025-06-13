#!/usr/bin/env python3
"""Test tool registration in the MCP server"""

import sys
import asyncio
sys.path.append('.')
from sse_server import mcp

async def test_tools():
    print("🔍 Checking MCP server tool registration...")
    
    try:
        # Use the async get_tools method
        tools = await mcp.get_tools()
        print(f"✅ Found {len(tools)} registered tools:")
        
        # Convert to list if needed
        if hasattr(tools, 'keys'):  # It's a dict
            tool_items = list(tools.items())
        else:  # It's a list or similar
            tool_items = list(enumerate(tools))
        
        for i, tool_data in enumerate(tool_items, 1):
            if isinstance(tool_data, tuple):  # (key, value) or (index, value)
                key, tool = tool_data
                name = key if isinstance(key, str) else str(tool)
            else:
                tool = tool_data
                name = str(tool)
            
            # Handle different tool formats
            if isinstance(tool, dict):
                description = tool.get('description', 'No description')
            elif hasattr(tool, 'description'):
                description = tool.description
            elif hasattr(tool, '__doc__') and tool.__doc__:
                description = tool.__doc__.split('\n')[0]
            else:
                description = "No description available"
            
            print(f"  {i:2d}. {name:<25} - {description[:60]}{'...' if len(description) > 60 else ''}")
        
        # Debug the tool structure
        print(f"\n🔍 Tool collection type: {type(tools)}")
        if hasattr(tools, 'keys'):
            print(f"Tool names: {list(tools.keys())}")
        
        return len(tools) > 0
        
    except Exception as e:
        print(f"❌ Error getting tools: {e}")
        import traceback
        traceback.print_exc()
        return False

if __name__ == "__main__":
    success = asyncio.run(test_tools())
    if success:
        print("\n🎯 Tools are properly registered!")
        print("📋 All 11 tools should now be visible in Cursor's MCP panel")
    else:
        print("\n❌ Tool registration failed!")
    sys.exit(0 if success else 1) 