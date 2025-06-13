# CURSOR AI AGENT TOOL USAGE OPTIMIZATION RULES
# Based on 2024-2025 Best Practices and Research

## CRITICAL TOOL EXECUTION PRINCIPLES

### 1. PARALLEL TOOL EXECUTION (HIGHEST PRIORITY)
**ALWAYS prioritize parallel tool calls over sequential execution unless there's a genuine dependency.**

- **DEFAULT TO PARALLEL**: Execute multiple independent operations simultaneously
- **3-5x PERFORMANCE GAIN**: Parallel execution dramatically improves response time
- **BATCH INFORMATION GATHERING**: Plan all searches upfront, execute together
- **EXAMPLES OF PARALLEL OPERATIONS**:
  - Multiple file reads
  - Different grep/search patterns
  - Codebase searches across different directories
  - Information gathering from multiple sources
  - File operations that don't depend on each other

**SEQUENTIAL ONLY WHEN**: Output of Tool A is required as input for Tool B

### 2. TOOL SELECTION EFFICIENCY
- **USE APPROPRIATE TOOLS**: Match tool capabilities to task requirements
- **AVOID TOOL MISUSE**: Don't manipulate tools for unintended purposes
- **MODULAR APPROACH**: Use independent tool components for better maintainability
- **CONTEXT AWARENESS**: Consider full context before tool selection

### 3. INFORMATION GATHERING STRATEGY
- **COMPREHENSIVE SEARCH**: Gather all needed information in one parallel batch
- **SEMANTIC + EXACT SEARCH**: Combine codebase_search with grep_search for complete coverage
- **DIRECTORY TARGETING**: Use specific directory patterns when scope is known
- **RESULT VALIDATION**: Verify search results completeness before proceeding

## TOOL-SPECIFIC OPTIMIZATION RULES

### FILE OPERATIONS
```
PARALLEL EXECUTION PATTERNS:
✅ read_file(file1) + read_file(file2) + read_file(file3)
✅ grep_search(pattern1) + grep_search(pattern2) + codebase_search(query)
✅ file_search(query1) + list_dir(path1) + list_dir(path2)

AVOID SEQUENTIAL:
❌ read_file(file1) → wait → read_file(file2) → wait → read_file(file3)
```

### SEARCH OPERATIONS
- **COMBINE SEARCH TYPES**: Use semantic search + regex search + file search in parallel
- **MULTIPLE PATTERNS**: Execute different search patterns simultaneously
- **CROSS-DIRECTORY**: Search multiple directories concurrently
- **RESULT AGGREGATION**: Process all search results together for comprehensive analysis

### CODE EDITING
- **BATCH EDITS**: Plan multiple file edits and execute in parallel when possible
- **CONTEXT GATHERING**: Read all relevant files before making changes
- **DEPENDENCY ANALYSIS**: Understand full codebase impact before modifications
- **VALIDATION**: Test changes immediately after implementation

## ANTI-PATTERNS TO AVOID

### 1. SEQUENTIAL TOOL ABUSE
```
❌ BAD PATTERN:
search_file(query1)
→ wait for result
→ read_file(found_file)
→ wait for result  
→ search_file(query2)
→ wait for result

✅ GOOD PATTERN:
search_file(query1) + search_file(query2) + codebase_search(related_query)
→ process all results
→ read_file(file1) + read_file(file2) + read_file(file3)
```

### 2. INCOMPLETE INFORMATION GATHERING
- **DON'T**: Make assumptions without sufficient context
- **DO**: Gather comprehensive information before proceeding
- **DON'T**: Stop at first search result if more context needed
- **DO**: Execute multiple search strategies in parallel

### 3. TOOL MISUSE PATTERNS
- **AVOID**: Using tools outside their intended scope
- **AVOID**: Manipulating tools for unintended actions
- **AVOID**: Ignoring tool parameter requirements
- **AVOID**: Making up parameter values when not provided

## EXECUTION WORKFLOW OPTIMIZATION

### 1. PLANNING PHASE
```
BEFORE TOOL EXECUTION:
1. Identify ALL information needed
2. Plan parallel tool execution strategy
3. Group independent operations
4. Identify genuine dependencies
5. Execute parallel batches
```

### 2. EXECUTION PHASE
```
PARALLEL EXECUTION CHECKLIST:
□ Can these operations run independently?
□ Do I need output from A to determine input for B?
□ Can I batch these information gathering tasks?
□ Am I maximizing parallel efficiency?
```

### 3. VALIDATION PHASE
```
POST-EXECUTION VALIDATION:
□ Did I gather sufficient context?
□ Are there gaps in information?
□ Do I need additional parallel searches?
□ Can I proceed with confidence?
```

## PERFORMANCE OPTIMIZATION RULES

### 1. MINIMIZE WAIT TIME
- **BATCH OPERATIONS**: Group similar operations together
- **PARALLEL BY DEFAULT**: Always consider parallel execution first
- **EFFICIENT PATTERNS**: Use proven parallel execution patterns
- **AVOID BLOCKING**: Don't wait for results when parallel execution possible

### 2. CONTEXT MANAGEMENT
- **COMPREHENSIVE GATHERING**: Get all context in parallel batches
- **SMART CACHING**: Reuse information from previous tool calls
- **SCOPE OPTIMIZATION**: Target searches to relevant directories/files
- **RESULT SYNTHESIS**: Combine multiple search results effectively

### 3. ERROR HANDLING
- **GRACEFUL DEGRADATION**: Handle tool failures without stopping workflow
- **RETRY STRATEGIES**: Implement smart retry for failed operations
- **FALLBACK PATTERNS**: Have alternative approaches ready
- **USER COMMUNICATION**: Clearly communicate tool execution status

## ADVANCED PATTERNS

### 1. MULTI-MODAL INFORMATION GATHERING
```
COMPREHENSIVE CODEBASE ANALYSIS:
codebase_search("feature implementation") +
grep_search("function_name\\(") +
file_search("feature") +
list_dir("src/features") +
read_file("README.md")
```

### 2. DEPENDENCY-AWARE EXECUTION
```
PHASE 1 (PARALLEL): Information gathering
PHASE 2 (PARALLEL): File reading based on Phase 1 results  
PHASE 3 (PARALLEL): Code modifications based on Phase 2 analysis
```

### 3. ITERATIVE REFINEMENT
- **BATCH VALIDATION**: Validate multiple assumptions in parallel
- **PROGRESSIVE ENHANCEMENT**: Build understanding through parallel exploration
- **FEEDBACK LOOPS**: Use parallel validation to confirm approaches

## QUALITY ASSURANCE

### 1. TOOL CALL VALIDATION
```
BEFORE EACH TOOL CALL:
□ Is this the most efficient approach?
□ Can I combine this with other operations?
□ Am I using the right tool for this task?
□ Have I provided all required parameters?
```

### 2. RESULT VERIFICATION
```
AFTER TOOL EXECUTION:
□ Did I get the expected results?
□ Is additional information needed?
□ Can I proceed with confidence?
□ Should I gather more context in parallel?
```

### 3. CONTINUOUS IMPROVEMENT
- **PATTERN RECOGNITION**: Identify successful parallel execution patterns
- **EFFICIENCY METRICS**: Monitor tool execution performance
- **BEST PRACTICE EVOLUTION**: Adapt patterns based on results
- **USER FEEDBACK INTEGRATION**: Incorporate user preferences and constraints

## EMERGENCY PROTOCOLS

### 1. TOOL FAILURE HANDLING
- **PARALLEL FALLBACKS**: Have alternative tools ready
- **GRACEFUL DEGRADATION**: Continue with available information
- **USER COMMUNICATION**: Explain limitations clearly
- **RECOVERY STRATEGIES**: Implement smart recovery patterns

### 2. PERFORMANCE ISSUES
- **TIMEOUT HANDLING**: Manage long-running operations
- **RESOURCE OPTIMIZATION**: Balance parallel execution with system limits
- **PRIORITY MANAGEMENT**: Focus on critical operations first
- **ADAPTIVE STRATEGIES**: Adjust approach based on performance

## IMPLEMENTATION CHECKLIST

### Daily Operation Checklist
```
□ Am I defaulting to parallel execution?
□ Have I planned my tool usage strategy?
□ Am I gathering comprehensive context?
□ Are my tool calls efficient and appropriate?
□ Am I validating results before proceeding?
□ Am I communicating clearly with the user?
```

### Quality Assurance Checklist
```
□ No unnecessary sequential operations
□ All required parameters provided
□ Appropriate tools selected for tasks
□ Comprehensive information gathering
□ Efficient parallel execution patterns
□ Clear communication of actions and results
```

---

**REMEMBER**: The goal is to be maximally helpful while being maximally efficient. Parallel tool execution is not just an optimization—it's the expected standard for professional AI assistance in 2025. 