"""Test file to verify Python development setup is working correctly."""

import sys
from typing import List


def add_numbers(a: int, b: int) -> int:
    """Add two numbers together.
    
    Args:
        a: First number
        b: Second number
        
    Returns:
        Sum of the two numbers
    """
    return a + b


def get_python_version() -> str:
    """Get the current Python version.
    
    Returns:
        Python version string
    """
    return f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"


def process_list(items: List[str]) -> List[str]:
    """Process a list of strings by converting to uppercase.
    
    Args:
        items: List of strings to process
        
    Returns:
        List of uppercase strings
    """
    return [item.upper() for item in items]


def test_add_numbers():
    """Test the add_numbers function."""
    assert add_numbers(2, 3) == 5
    assert add_numbers(-1, 1) == 0
    assert add_numbers(0, 0) == 0


def test_get_python_version():
    """Test the get_python_version function."""
    version = get_python_version()
    assert isinstance(version, str)
    assert len(version.split('.')) == 3


def test_process_list():
    """Test the process_list function."""
    input_list = ["hello", "world", "python"]
    expected = ["HELLO", "WORLD", "PYTHON"]
    assert process_list(input_list) == expected
    
    # Test empty list
    assert process_list([]) == []


if __name__ == "__main__":
    print(f"Python version: {get_python_version()}")
    print(f"2 + 3 = {add_numbers(2, 3)}")
    print(f"Processed list: {process_list(['hello', 'world'])}")
    print("All manual tests passed!") 