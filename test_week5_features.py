#!/usr/bin/env python3
"""
Week 5 Advanced Features Testing Suite
Comprehensive testing for performance optimization, caching, and advanced analytics
"""

import asyncio
import json
import time
import sys
import os
from typing import Dict, List, Any
from datetime import datetime
import logging

# Add the current directory to Python path to import sse_server
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("week5_tests")

# ========================================
# 🧪 WEEK 5 TESTING INFRASTRUCTURE
# ========================================

class Week5TestSuite:
    """Comprehensive testing suite for Week 5 advanced features"""
    
    def __init__(self):
        self.test_results = {}
        
    async def setup(self):
        """Initialize test session"""
        logger.info("🧪 Week 5 Test Suite initialized")
    
    async def teardown(self):
        """Cleanup test session"""
        logger.info("✅ Test suite cleanup completed")
    
    async def call_mcp_tool(self, tool_name: str, params: Dict[str, Any]) -> Dict[str, Any]:
        """Call MCP tool directly for testing"""
        try:
            # Import the actual tools for testing
            from sse_server import (
                find_similar_concepts, analyze_patterns, 
                get_semantic_clusters, get_temporal_patterns,
                get_performance_stats, clear_performance_cache
            )
            
            # Route to appropriate tool
            if tool_name == "find_similar_concepts":
                result = await find_similar_concepts(**params)
            elif tool_name == "analyze_patterns":
                result = await analyze_patterns(**params)
            elif tool_name == "get_semantic_clusters":
                result = await get_semantic_clusters(**params)
            elif tool_name == "get_temporal_patterns":
                result = await get_temporal_patterns(**params)
            elif tool_name == "get_performance_stats":
                result = await get_performance_stats()
            elif tool_name == "clear_performance_cache":
                result = await clear_performance_cache()
            else:
                raise ValueError(f"Unknown tool: {tool_name}")
            
            return json.loads(result)
            
        except Exception as e:
            logger.error(f"Tool call failed: {e}")
            return {"error": str(e)}

    # ========================================
    # 🎯 ADVANCED FEATURES TESTING
    # ========================================
    
    async def test_find_similar_concepts(self):
        """Test semantic similarity search functionality"""
        logger.info("🔍 Testing find_similar_concepts...")
        
        test_cases = [
            {
                "concept": "artificial intelligence",
                "max_results": 5,
                "similarity_threshold": 0.8
            },
            {
                "concept": "machine learning",
                "max_results": 10,
                "similarity_threshold": 0.7
            },
            {
                "concept": "quantum computing",
                "max_results": 3,
                "similarity_threshold": 0.9
            }
        ]
        
        results = []
        for test_case in test_cases:
            start_time = time.time()
            
            result = await self.call_mcp_tool("find_similar_concepts", test_case)
            
            end_time = time.time()
            response_time = (end_time - start_time) * 1000  # Convert to ms
            
            # Validate response structure
            assert "success" in result
            assert "similar_concepts" in result
            assert "concept" in result
            assert result["concept"] == test_case["concept"]
            
            # Validate performance
            assert response_time < 1000, f"Response time too slow: {response_time}ms"
            
            # Validate similarity scores
            for concept in result.get("similar_concepts", []):
                assert "similarity" in concept
                assert concept["similarity"] >= test_case["similarity_threshold"]
                assert concept["similarity"] <= 1.0
            
            results.append({
                "test_case": test_case,
                "result": result,
                "response_time_ms": response_time,
                "passed": True
            })
            
            logger.info(f"   ✅ Concept '{test_case['concept']}' - {response_time:.1f}ms")
        
        self.test_results["find_similar_concepts"] = results
        return results
    
    async def test_analyze_patterns(self):
        """Test pattern analysis functionality"""
        logger.info("📊 Testing analyze_patterns...")
        
        analysis_types = ["relationships", "clusters", "temporal", "centrality"]
        results = []
        
        for analysis_type in analysis_types:
            start_time = time.time()
            
            params = {
                "analysis_type": analysis_type,
                "max_results": 15,
                "time_range_days": 30 if analysis_type == "temporal" else None
            }
            
            result = await self.call_mcp_tool("analyze_patterns", params)
            
            end_time = time.time()
            response_time = (end_time - start_time) * 1000
            
            # Validate response structure
            assert "success" in result
            assert "analysis_type" in result
            assert "patterns" in result
            assert result["analysis_type"] == analysis_type
            
            # Validate performance
            assert response_time < 2000, f"Analysis too slow: {response_time}ms"
            
            # Validate pattern structure based on type
            patterns = result.get("patterns", [])
            if analysis_type == "relationships" and patterns:
                assert "pattern" in patterns[0]
                assert "frequency" in patterns[0]
                assert "strength" in patterns[0]
            elif analysis_type == "clusters" and patterns:
                assert "cluster_id" in patterns[0]
                assert "size" in patterns[0]
                assert "coherence" in patterns[0]
            elif analysis_type == "temporal" and patterns:
                assert "time_period" in patterns[0]
                assert "activity_level" in patterns[0]
                assert "trend" in patterns[0]
            elif analysis_type == "centrality" and patterns:
                assert "node" in patterns[0]
                assert "degree_centrality" in patterns[0]
            
            results.append({
                "analysis_type": analysis_type,
                "result": result,
                "response_time_ms": response_time,
                "passed": True
            })
            
            logger.info(f"   ✅ {analysis_type} analysis - {response_time:.1f}ms")
        
        self.test_results["analyze_patterns"] = results
        return results
    
    async def test_semantic_clustering(self):
        """Test semantic clustering functionality"""
        logger.info("🎯 Testing get_semantic_clusters...")
        
        cluster_methods = ["kmeans", "hierarchical", "dbscan"]
        results = []
        
        for method in cluster_methods:
            start_time = time.time()
            
            params = {
                "cluster_method": method,
                "num_clusters": 5,
                "min_cluster_size": 3
            }
            
            result = await self.call_mcp_tool("get_semantic_clusters", params)
            
            end_time = time.time()
            response_time = (end_time - start_time) * 1000
            
            # Validate response structure
            assert "success" in result
            assert "clusters" in result
            assert "cluster_method" in result
            assert result["cluster_method"] == method
            
            # Validate performance
            assert response_time < 3000, f"Clustering too slow: {response_time}ms"
            
            # Validate cluster structure
            clusters = result.get("clusters", [])
            for cluster in clusters:
                assert "cluster_id" in cluster
                assert "size" in cluster
                assert "coherence_score" in cluster
                assert cluster["size"] >= params["min_cluster_size"]
                assert 0 <= cluster["coherence_score"] <= 1
            
            results.append({
                "method": method,
                "result": result,
                "response_time_ms": response_time,
                "passed": True
            })
            
            logger.info(f"   ✅ {method} clustering - {response_time:.1f}ms")
        
        self.test_results["semantic_clustering"] = results
        return results
    
    async def test_temporal_patterns(self):
        """Test temporal pattern analysis"""
        logger.info("⏰ Testing get_temporal_patterns...")
        
        granularities = ["day", "week", "month"]
        results = []
        
        for granularity in granularities:
            start_time = time.time()
            
            params = {
                "time_granularity": granularity,
                "days_back": 30,
                "concept_filter": None
            }
            
            result = await self.call_mcp_tool("get_temporal_patterns", params)
            
            end_time = time.time()
            response_time = (end_time - start_time) * 1000
            
            # Validate response structure
            assert "success" in result
            assert "patterns" in result
            assert "time_granularity" in result
            assert result["time_granularity"] == granularity
            
            # Validate performance
            assert response_time < 2000, f"Temporal analysis too slow: {response_time}ms"
            
            # Validate pattern structure
            patterns = result.get("patterns", [])
            for pattern in patterns:
                assert "time_period" in pattern
                assert "activity_level" in pattern
                assert "trend" in pattern
                assert 0 <= pattern["activity_level"] <= 1
                assert pattern["trend"] in ["increasing", "stable", "decreasing"]
            
            results.append({
                "granularity": granularity,
                "result": result,
                "response_time_ms": response_time,
                "passed": True
            })
            
            logger.info(f"   ✅ {granularity} granularity - {response_time:.1f}ms")
        
        self.test_results["temporal_patterns"] = results
        return results

    # ========================================
    # 🚀 PERFORMANCE TESTING
    # ========================================
    
    async def test_cache_performance(self):
        """Test caching system performance and hit rates"""
        logger.info("💾 Testing cache performance...")
        
        # Clear cache first
        await self.call_mcp_tool("clear_performance_cache", {})
        
        # Test cache miss and population
        concept = "machine learning"
        params = {"concept": concept, "max_results": 5}
        
        # First call - should be cache miss
        start_time = time.time()
        result1 = await self.call_mcp_tool("find_similar_concepts", params)
        first_call_time = (time.time() - start_time) * 1000
        
        assert result1.get("cache_status") == "miss"
        
        # Second call - should be cache hit
        start_time = time.time()
        result2 = await self.call_mcp_tool("find_similar_concepts", params)
        second_call_time = (time.time() - start_time) * 1000
        
        assert result2.get("cache_status") == "hit"
        
        # Cache hit should be significantly faster
        speed_improvement = first_call_time / second_call_time if second_call_time > 0 else float('inf')
        assert speed_improvement > 2, f"Cache not improving performance sufficiently: {speed_improvement}x"
        
        # Get performance stats
        stats = await self.call_mcp_tool("get_performance_stats", {})
        cache_stats = stats.get("cache_performance", {})
        
        assert cache_stats.get("hits") > 0
        assert cache_stats.get("hit_rate") > 0
        
        results = {
            "first_call_ms": first_call_time,
            "second_call_ms": second_call_time,
            "speed_improvement": speed_improvement,
            "cache_stats": cache_stats,
            "passed": True
        }
        
        logger.info(f"   ✅ Cache performance: {speed_improvement:.1f}x improvement")
        
        self.test_results["cache_performance"] = results
        return results
    
    async def test_concurrent_requests(self):
        """Test concurrent request handling"""
        logger.info("🔄 Testing concurrent request handling...")
        
        # Create multiple concurrent requests
        concepts = [
            "artificial intelligence",
            "machine learning", 
            "data science",
            "neural networks",
            "deep learning"
        ]
        
        async def make_request(concept):
            start_time = time.time()
            result = await self.call_mcp_tool("find_similar_concepts", {
                "concept": concept,
                "max_results": 3
            })
            end_time = time.time()
            return {
                "concept": concept,
                "result": result,
                "response_time_ms": (end_time - start_time) * 1000
            }
        
        # Execute all requests concurrently
        start_time = time.time()
        tasks = [make_request(concept) for concept in concepts]
        results = await asyncio.gather(*tasks)
        total_time = (time.time() - start_time) * 1000
        
        # Validate all requests succeeded
        for result in results:
            assert result["result"].get("success") == True
            assert result["response_time_ms"] < 2000  # Each request under 2s
        
        # Average response time should be reasonable
        avg_response_time = sum(r["response_time_ms"] for r in results) / len(results)
        assert avg_response_time < 1000, f"Average response time too slow: {avg_response_time}ms"
        
        concurrent_results = {
            "total_requests": len(concepts),
            "total_time_ms": total_time,
            "average_response_time_ms": avg_response_time,
            "max_response_time_ms": max(r["response_time_ms"] for r in results),
            "min_response_time_ms": min(r["response_time_ms"] for r in results),
            "passed": True
        }
        
        logger.info(f"   ✅ Concurrent requests: {len(concepts)} requests in {total_time:.1f}ms")
        
        self.test_results["concurrent_requests"] = concurrent_results
        return concurrent_results

    # ========================================
    # 🏋️ STRESS TESTING
    # ========================================
    
    async def test_high_load_performance(self):
        """Test system performance under high load"""
        logger.info("🏋️ Testing high load performance...")
        
        # Test with rapid successive requests
        num_requests = 50
        concept = "performance testing"
        
        results = []
        start_time = time.time()
        
        for i in range(num_requests):
            request_start = time.time()
            
            result = await self.call_mcp_tool("find_similar_concepts", {
                "concept": f"{concept} {i}",
                "max_results": 3
            })
            
            request_time = (time.time() - request_start) * 1000
            results.append({
                "request_id": i,
                "response_time_ms": request_time,
                "success": result.get("success", False)
            })
        
        total_time = (time.time() - start_time) * 1000
        
        # Calculate performance metrics
        successful_requests = sum(1 for r in results if r["success"])
        success_rate = successful_requests / num_requests
        avg_response_time = sum(r["response_time_ms"] for r in results) / len(results)
        max_response_time = max(r["response_time_ms"] for r in results)
        
        # Performance requirements
        assert success_rate > 0.95, f"Success rate too low: {success_rate}"
        assert avg_response_time < 500, f"Average response time too slow: {avg_response_time}ms"
        assert max_response_time < 2000, f"Max response time too slow: {max_response_time}ms"
        
        load_test_results = {
            "total_requests": num_requests,
            "successful_requests": successful_requests,
            "success_rate": success_rate,
            "total_time_ms": total_time,
            "avg_response_time_ms": avg_response_time,
            "max_response_time_ms": max_response_time,
            "requests_per_second": num_requests / (total_time / 1000),
            "passed": True
        }
        
        logger.info(f"   ✅ High load: {success_rate*100:.1f}% success rate, {avg_response_time:.1f}ms avg")
        
        self.test_results["high_load_performance"] = load_test_results
        return load_test_results

    # ========================================
    # 📊 COMPREHENSIVE TEST RUNNER
    # ========================================
    
    async def run_all_tests(self):
        """Run comprehensive Week 5 test suite"""
        logger.info("🧪 Starting Week 5 Comprehensive Test Suite...")
        
        await self.setup()
        
        try:
            # Advanced Features Tests
            await self.test_find_similar_concepts()
            await self.test_analyze_patterns()
            await self.test_semantic_clustering()
            await self.test_temporal_patterns()
            
            # Performance Tests
            await self.test_cache_performance()
            await self.test_concurrent_requests()
            await self.test_high_load_performance()
            
            # Generate test report
            await self.generate_test_report()
            
        except Exception as e:
            logger.error(f"Test suite failed: {e}")
            raise
        finally:
            await self.teardown()
    
    async def generate_test_report(self):
        """Generate comprehensive test report"""
        logger.info("📊 Generating Week 5 Test Report...")
        
        total_tests = len(self.test_results)
        passed_tests = sum(1 for test_group in self.test_results.values() 
                          if isinstance(test_group, dict) and test_group.get("passed")
                          or isinstance(test_group, list) and all(t.get("passed", False) for t in test_group))
        
        success_rate = passed_tests / total_tests if total_tests > 0 else 0
        
        report = {
            "test_suite": "Week 5 Advanced Features",
            "timestamp": datetime.now().isoformat(),
            "summary": {
                "total_test_groups": total_tests,
                "passed_test_groups": passed_tests,
                "success_rate": success_rate,
                "overall_status": "PASSED" if success_rate == 1.0 else "FAILED"
            },
            "detailed_results": self.test_results,
            "performance_summary": {
                "cache_enabled": True,
                "concurrent_support": True,
                "high_load_capable": True,
                "advanced_analytics": True
            }
        }
        
        # Save report to file
        with open("week5_test_report.json", "w") as f:
            json.dump(report, f, indent=2, default=str)
        
        logger.info(f"✅ Test Report Generated:")
        logger.info(f"   • Total Test Groups: {total_tests}")
        logger.info(f"   • Success Rate: {success_rate*100:.1f}%")
        logger.info(f"   • Overall Status: {report['summary']['overall_status']}")
        logger.info(f"   • Report saved to: week5_test_report.json")
        
        return report

# ========================================
# 🚀 MAIN TEST EXECUTION
# ========================================

async def main():
    """Main test execution function"""
    test_suite = Week5TestSuite()
    await test_suite.run_all_tests()

if __name__ == "__main__":
    asyncio.run(main()) 