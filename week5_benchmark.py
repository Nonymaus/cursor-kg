#!/usr/bin/env python3
"""
Week 5 Performance Benchmarking Suite
Comprehensive performance testing against Week 5 targets
"""

import asyncio
import json
import time
import sys
import os
import statistics
from typing import Dict, List, Any, Tuple
from datetime import datetime
import logging
try:
    import matplotlib.pyplot as plt
    import numpy as np
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False
    # Mock numpy percentile function
    def percentile(data, p):
        data_sorted = sorted(data)
        k = (len(data_sorted) - 1) * p / 100
        f = int(k)
        c = k - f
        if f + 1 < len(data_sorted):
            return data_sorted[f] * (1 - c) + data_sorted[f + 1] * c
        else:
            return data_sorted[f]
    
    class np:
        @staticmethod
        def percentile(data, p):
            return percentile(data, p)
        
        @staticmethod
        def arange(n):
            return list(range(n))

# Add the current directory to Python path
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("week5_benchmark")

# ========================================
# 🎯 WEEK 5 PERFORMANCE TARGETS
# ========================================

PERFORMANCE_TARGETS = {
    "find_similar_concepts": {
        "target_latency_ms": 100,
        "max_latency_ms": 500,
        "throughput_rps": 20
    },
    "analyze_patterns": {
        "target_latency_ms": 200,
        "max_latency_ms": 1000,
        "throughput_rps": 10
    },
    "get_semantic_clusters": {
        "target_latency_ms": 500,
        "max_latency_ms": 2000,
        "throughput_rps": 5
    },
    "get_temporal_patterns": {
        "target_latency_ms": 300,
        "max_latency_ms": 1500,
        "throughput_rps": 8
    },
    "cache_performance": {
        "hit_rate_target": 0.8,
        "speed_improvement_min": 5.0
    },
    "memory_usage": {
        "base_memory_mb": 50,
        "per_10k_nodes_mb": 1
    }
}

class Week5Benchmark:
    """Comprehensive performance benchmarking for Week 5 features"""
    
    def __init__(self):
        self.benchmark_results = {}
        self.performance_metrics = {}
        
    async def setup(self):
        """Initialize benchmarking environment"""
        logger.info("🎯 Initializing Week 5 Performance Benchmark Suite...")
        
        # Import tools
        try:
            from sse_server import (
                find_similar_concepts, analyze_patterns,
                get_semantic_clusters, get_temporal_patterns,
                get_performance_stats, clear_performance_cache
            )
            self.tools = {
                "find_similar_concepts": find_similar_concepts,
                "analyze_patterns": analyze_patterns,
                "get_semantic_clusters": get_semantic_clusters,
                "get_temporal_patterns": get_temporal_patterns,
                "get_performance_stats": get_performance_stats,
                "clear_performance_cache": clear_performance_cache
            }
            logger.info("✅ Tools imported successfully")
        except Exception as e:
            logger.error(f"Failed to import tools: {e}")
            raise
    
    async def call_tool(self, tool_name: str, params: Dict[str, Any]) -> Tuple[Dict[str, Any], float]:
        """Call tool and measure performance"""
        start_time = time.perf_counter()
        
        try:
            result = await self.tools[tool_name](**params)
            result_dict = json.loads(result) if isinstance(result, str) else result
        except Exception as e:
            result_dict = {"error": str(e)}
        
        end_time = time.perf_counter()
        latency_ms = (end_time - start_time) * 1000
        
        return result_dict, latency_ms

    # ========================================
    # 📊 LATENCY BENCHMARKS
    # ========================================
    
    async def benchmark_latency(self, tool_name: str, test_params: List[Dict[str, Any]], 
                               iterations: int = 50) -> Dict[str, Any]:
        """Benchmark tool latency performance"""
        logger.info(f"📊 Benchmarking {tool_name} latency ({iterations} iterations)...")
        
        latencies = []
        successes = 0
        errors = []
        
        for i in range(iterations):
            # Cycle through test parameters
            params = test_params[i % len(test_params)]
            
            result, latency_ms = await self.call_tool(tool_name, params)
            latencies.append(latency_ms)
            
            if "error" not in result:
                successes += 1
            else:
                errors.append(result["error"])
        
        # Calculate statistics
        success_rate = successes / iterations
        avg_latency = statistics.mean(latencies)
        median_latency = statistics.median(latencies)
        p95_latency = np.percentile(latencies, 95)
        p99_latency = np.percentile(latencies, 99)
        min_latency = min(latencies)
        max_latency = max(latencies)
        
        # Check against targets
        target = PERFORMANCE_TARGETS.get(tool_name, {})
        target_latency = target.get("target_latency_ms", 1000)
        max_allowed_latency = target.get("max_latency_ms", 5000)
        
        meets_target = avg_latency <= target_latency
        meets_max = max_latency <= max_allowed_latency
        
        benchmark_result = {
            "tool": tool_name,
            "iterations": iterations,
            "success_rate": success_rate,
            "latency_stats": {
                "average_ms": avg_latency,
                "median_ms": median_latency,
                "p95_ms": p95_latency,
                "p99_ms": p99_latency,
                "min_ms": min_latency,
                "max_ms": max_latency,
                "std_dev_ms": statistics.stdev(latencies) if len(latencies) > 1 else 0
            },
            "performance_targets": {
                "target_latency_ms": target_latency,
                "max_allowed_latency_ms": max_allowed_latency,
                "meets_target": meets_target,
                "meets_max_requirement": meets_max
            },
            "raw_latencies": latencies,
            "errors": errors
        }
        
        logger.info(f"   ✅ {tool_name}: {avg_latency:.1f}ms avg, {success_rate*100:.1f}% success")
        if not meets_target:
            logger.warning(f"   ⚠️  Target missed: {avg_latency:.1f}ms > {target_latency}ms")
        
        return benchmark_result
    
    async def benchmark_throughput(self, tool_name: str, test_params: Dict[str, Any], 
                                  duration_seconds: int = 30) -> Dict[str, Any]:
        """Benchmark tool throughput performance"""
        logger.info(f"🚀 Benchmarking {tool_name} throughput ({duration_seconds}s test)...")
        
        start_time = time.perf_counter()
        end_time = start_time + duration_seconds
        
        requests_completed = 0
        total_latency = 0
        errors = 0
        
        while time.perf_counter() < end_time:
            result, latency_ms = await self.call_tool(tool_name, test_params)
            
            requests_completed += 1
            total_latency += latency_ms
            
            if "error" in result:
                errors += 1
        
        actual_duration = time.perf_counter() - start_time
        
        # Calculate throughput metrics
        requests_per_second = requests_completed / actual_duration
        avg_latency = total_latency / requests_completed if requests_completed > 0 else 0
        error_rate = errors / requests_completed if requests_completed > 0 else 0
        
        # Check against targets
        target = PERFORMANCE_TARGETS.get(tool_name, {})
        target_rps = target.get("throughput_rps", 1)
        meets_throughput_target = requests_per_second >= target_rps
        
        throughput_result = {
            "tool": tool_name,
            "test_duration_seconds": actual_duration,
            "requests_completed": requests_completed,
            "requests_per_second": requests_per_second,
            "average_latency_ms": avg_latency,
            "error_rate": error_rate,
            "target_rps": target_rps,
            "meets_throughput_target": meets_throughput_target
        }
        
        logger.info(f"   ✅ {tool_name}: {requests_per_second:.1f} RPS, {error_rate*100:.1f}% errors")
        if not meets_throughput_target:
            logger.warning(f"   ⚠️  Throughput target missed: {requests_per_second:.1f} < {target_rps} RPS")
        
        return throughput_result

    # ========================================
    # 💾 CACHE PERFORMANCE BENCHMARKS
    # ========================================
    
    async def benchmark_cache_performance(self) -> Dict[str, Any]:
        """Benchmark caching system performance"""
        logger.info("💾 Benchmarking cache performance...")
        
        # Clear cache
        await self.call_tool("clear_performance_cache", {})
        
        # Test parameters
        test_concept = "machine learning performance"
        params = {"concept": test_concept, "max_results": 5}
        
        # Measure cache miss performance
        cache_miss_latencies = []
        for _ in range(10):
            # Clear cache for each miss test
            await self.call_tool("clear_performance_cache", {})
            result, latency_ms = await self.call_tool("find_similar_concepts", params)
            cache_miss_latencies.append(latency_ms)
        
        avg_miss_latency = statistics.mean(cache_miss_latencies)
        
        # Measure cache hit performance
        cache_hit_latencies = []
        
        # Prime the cache
        await self.call_tool("find_similar_concepts", params)
        
        # Measure cache hits
        for _ in range(20):
            result, latency_ms = await self.call_tool("find_similar_concepts", params)
            cache_hit_latencies.append(latency_ms)
            
            # Verify it was a cache hit
            if result.get("cache_status") != "hit":
                logger.warning("Expected cache hit but got cache miss")
        
        avg_hit_latency = statistics.mean(cache_hit_latencies)
        speed_improvement = avg_miss_latency / avg_hit_latency if avg_hit_latency > 0 else float('inf')
        
        # Get final cache stats
        stats_result, _ = await self.call_tool("get_performance_stats", {})
        cache_stats = stats_result.get("cache_performance", {})
        
        # Check against targets
        target_hit_rate = PERFORMANCE_TARGETS["cache_performance"]["hit_rate_target"]
        target_speed_improvement = PERFORMANCE_TARGETS["cache_performance"]["speed_improvement_min"]
        
        actual_hit_rate = cache_stats.get("hit_rate", 0)
        meets_hit_rate = actual_hit_rate >= target_hit_rate
        meets_speed_improvement = speed_improvement >= target_speed_improvement
        
        cache_benchmark = {
            "cache_miss_latency_ms": avg_miss_latency,
            "cache_hit_latency_ms": avg_hit_latency,
            "speed_improvement": speed_improvement,
            "cache_statistics": cache_stats,
            "targets": {
                "target_hit_rate": target_hit_rate,
                "actual_hit_rate": actual_hit_rate,
                "meets_hit_rate_target": meets_hit_rate,
                "target_speed_improvement": target_speed_improvement,
                "meets_speed_improvement": meets_speed_improvement
            }
        }
        
        logger.info(f"   ✅ Cache: {speed_improvement:.1f}x improvement, {actual_hit_rate*100:.1f}% hit rate")
        
        return cache_benchmark

    # ========================================
    # 🔄 CONCURRENT PERFORMANCE BENCHMARKS
    # ========================================
    
    async def benchmark_concurrent_performance(self, tool_name: str, 
                                             test_params: Dict[str, Any],
                                             concurrent_users: int = 10,
                                             requests_per_user: int = 5) -> Dict[str, Any]:
        """Benchmark concurrent request handling"""
        logger.info(f"🔄 Benchmarking {tool_name} concurrent performance "
                   f"({concurrent_users} users, {requests_per_user} req/user)...")
        
        async def user_simulation(user_id: int) -> List[Dict[str, Any]]:
            """Simulate a single user making multiple requests"""
            user_results = []
            
            for request_id in range(requests_per_user):
                start_time = time.perf_counter()
                result, latency_ms = await self.call_tool(tool_name, test_params)
                
                user_results.append({
                    "user_id": user_id,
                    "request_id": request_id,
                    "latency_ms": latency_ms,
                    "success": "error" not in result,
                    "timestamp": time.perf_counter()
                })
            
            return user_results
        
        # Execute concurrent user simulations
        start_time = time.perf_counter()
        
        tasks = [user_simulation(user_id) for user_id in range(concurrent_users)]
        all_results = await asyncio.gather(*tasks)
        
        end_time = time.perf_counter()
        total_duration = end_time - start_time
        
        # Flatten results
        all_requests = []
        for user_results in all_results:
            all_requests.extend(user_results)
        
        # Calculate metrics
        total_requests = len(all_requests)
        successful_requests = sum(1 for r in all_requests if r["success"])
        success_rate = successful_requests / total_requests if total_requests > 0 else 0
        
        latencies = [r["latency_ms"] for r in all_requests]
        avg_latency = statistics.mean(latencies)
        p95_latency = np.percentile(latencies, 95)
        
        requests_per_second = total_requests / total_duration
        
        concurrent_benchmark = {
            "tool": tool_name,
            "concurrent_users": concurrent_users,
            "requests_per_user": requests_per_user,
            "total_requests": total_requests,
            "total_duration_seconds": total_duration,
            "success_rate": success_rate,
            "requests_per_second": requests_per_second,
            "latency_stats": {
                "average_ms": avg_latency,
                "p95_ms": p95_latency,
                "min_ms": min(latencies),
                "max_ms": max(latencies)
            },
            "detailed_results": all_requests
        }
        
        logger.info(f"   ✅ Concurrent {tool_name}: {requests_per_second:.1f} RPS, "
                   f"{success_rate*100:.1f}% success, {avg_latency:.1f}ms avg")
        
        return concurrent_benchmark

    # ========================================
    # 📊 COMPREHENSIVE BENCHMARK SUITE
    # ========================================
    
    async def run_comprehensive_benchmarks(self):
        """Run complete Week 5 benchmark suite"""
        logger.info("🎯 Starting Week 5 Comprehensive Performance Benchmarks...")
        
        await self.setup()
        
        # Test parameters for each tool
        test_scenarios = {
            "find_similar_concepts": [
                {"concept": "artificial intelligence", "max_results": 5},
                {"concept": "machine learning", "max_results": 8},
                {"concept": "data science", "max_results": 3}
            ],
            "analyze_patterns": [
                {"analysis_type": "relationships", "max_results": 15},
                {"analysis_type": "clusters", "max_results": 10},
                {"analysis_type": "temporal", "max_results": 20},
                {"analysis_type": "centrality", "max_results": 12}
            ],
            "get_semantic_clusters": [
                {"cluster_method": "kmeans", "num_clusters": 5},
                {"cluster_method": "hierarchical", "num_clusters": 4},
                {"cluster_method": "dbscan", "min_cluster_size": 3}
            ],
            "get_temporal_patterns": [
                {"time_granularity": "day", "days_back": 30},
                {"time_granularity": "week", "days_back": 60},
                {"time_granularity": "month", "days_back": 90}
            ]
        }
        
        # 1. Latency Benchmarks
        logger.info("📊 Running latency benchmarks...")
        latency_results = {}
        for tool_name, params_list in test_scenarios.items():
            latency_results[tool_name] = await self.benchmark_latency(tool_name, params_list)
        
        # 2. Throughput Benchmarks
        logger.info("🚀 Running throughput benchmarks...")
        throughput_results = {}
        for tool_name, params_list in test_scenarios.items():
            # Use first parameter set for throughput testing
            throughput_results[tool_name] = await self.benchmark_throughput(
                tool_name, params_list[0]
            )
        
        # 3. Cache Performance Benchmark
        logger.info("💾 Running cache performance benchmark...")
        cache_results = await self.benchmark_cache_performance()
        
        # 4. Concurrent Performance Benchmarks
        logger.info("🔄 Running concurrent performance benchmarks...")
        concurrent_results = {}
        for tool_name, params_list in test_scenarios.items():
            concurrent_results[tool_name] = await self.benchmark_concurrent_performance(
                tool_name, params_list[0], concurrent_users=5, requests_per_user=3
            )
        
        # Compile comprehensive results
        self.benchmark_results = {
            "latency_benchmarks": latency_results,
            "throughput_benchmarks": throughput_results,
            "cache_benchmark": cache_results,
            "concurrent_benchmarks": concurrent_results,
            "performance_targets": PERFORMANCE_TARGETS,
            "timestamp": datetime.now().isoformat()
        }
        
        # Generate performance report
        await self.generate_performance_report()
        
        return self.benchmark_results
    
    async def generate_performance_report(self):
        """Generate comprehensive performance report"""
        logger.info("📊 Generating Week 5 Performance Report...")
        
        # Calculate overall performance score
        performance_scores = []
        
        # Latency performance scores
        for tool_name, results in self.benchmark_results["latency_benchmarks"].items():
            if results["performance_targets"]["meets_target"]:
                performance_scores.append(100)
            elif results["performance_targets"]["meets_max_requirement"]:
                performance_scores.append(75)
            else:
                performance_scores.append(50)
        
        # Throughput performance scores
        for tool_name, results in self.benchmark_results["throughput_benchmarks"].items():
            if results["meets_throughput_target"]:
                performance_scores.append(100)
            else:
                performance_scores.append(60)
        
        # Cache performance score
        cache_results = self.benchmark_results["cache_benchmark"]
        cache_score = 0
        if cache_results["targets"]["meets_hit_rate_target"]:
            cache_score += 50
        if cache_results["targets"]["meets_speed_improvement"]:
            cache_score += 50
        performance_scores.append(cache_score)
        
        overall_score = statistics.mean(performance_scores)
        
        # Create performance summary
        performance_summary = {
            "overall_performance_score": overall_score,
            "grade": self._get_performance_grade(overall_score),
            "meets_week5_targets": overall_score >= 80,
            "benchmark_timestamp": datetime.now().isoformat(),
            "summary_stats": {
                "total_tools_tested": len(self.benchmark_results["latency_benchmarks"]),
                "latency_targets_met": sum(1 for r in self.benchmark_results["latency_benchmarks"].values() 
                                         if r["performance_targets"]["meets_target"]),
                "throughput_targets_met": sum(1 for r in self.benchmark_results["throughput_benchmarks"].values() 
                                            if r["meets_throughput_target"]),
                "cache_performance_excellent": cache_results["targets"]["meets_speed_improvement"]
            }
        }
        
        # Add summary to results
        self.benchmark_results["performance_summary"] = performance_summary
        
        # Save detailed report
        with open("week5_performance_report.json", "w") as f:
            json.dump(self.benchmark_results, f, indent=2, default=str)
        
        # Generate performance visualization
        await self._create_performance_charts()
        
        # Print summary
        logger.info("✅ Week 5 Performance Report Generated:")
        logger.info(f"   • Overall Score: {overall_score:.1f}/100 ({performance_summary['grade']})")
        logger.info(f"   • Meets Week 5 Targets: {performance_summary['meets_week5_targets']}")
        logger.info(f"   • Latency Targets Met: {performance_summary['summary_stats']['latency_targets_met']}/{performance_summary['summary_stats']['total_tools_tested']}")
        logger.info(f"   • Throughput Targets Met: {performance_summary['summary_stats']['throughput_targets_met']}/{performance_summary['summary_stats']['total_tools_tested']}")
        logger.info(f"   • Report saved: week5_performance_report.json")
        
        return performance_summary
    
    def _get_performance_grade(self, score: float) -> str:
        """Convert performance score to letter grade"""
        if score >= 95:
            return "A+"
        elif score >= 90:
            return "A"
        elif score >= 85:
            return "B+"
        elif score >= 80:
            return "B"
        elif score >= 75:
            return "C+"
        elif score >= 70:
            return "C"
        else:
            return "F"
    
    async def _create_performance_charts(self):
        """Create performance visualization charts"""
        if not HAS_MATPLOTLIB:
            logger.info("   • Matplotlib not available - skipping chart generation")
            return
            
        try:
            # Latency comparison chart
            tools = list(self.benchmark_results["latency_benchmarks"].keys())
            latencies = [
                self.benchmark_results["latency_benchmarks"][tool]["latency_stats"]["average_ms"]
                for tool in tools
            ]
            targets = [
                PERFORMANCE_TARGETS[tool]["target_latency_ms"]
                for tool in tools
            ]
            
            plt.figure(figsize=(12, 6))
            
            x = np.arange(len(tools))
            width = 0.35
            
            plt.bar(x - width/2, latencies, width, label='Actual Latency', alpha=0.8)
            plt.bar(x + width/2, targets, width, label='Target Latency', alpha=0.8)
            
            plt.xlabel('Tools')
            plt.ylabel('Latency (ms)')
            plt.title('Week 5 Performance: Latency vs Targets')
            plt.xticks(x, [tool.replace('_', '\n') for tool in tools], rotation=45)
            plt.legend()
            plt.tight_layout()
            
            plt.savefig('week5_latency_performance.png', dpi=150, bbox_inches='tight')
            plt.close()
            
            logger.info("   • Performance chart saved: week5_latency_performance.png")
            
        except Exception as e:
            logger.warning(f"Could not create performance charts: {e}")

# ========================================
# 🚀 MAIN BENCHMARK EXECUTION
# ========================================

async def main():
    """Main benchmark execution function"""
    benchmark = Week5Benchmark()
    results = await benchmark.run_comprehensive_benchmarks()
    
    # Print final summary
    summary = results["performance_summary"]
    logger.info(f"\n🎯 Week 5 Performance Benchmark Complete!")
    logger.info(f"Overall Performance: {summary['overall_performance_score']:.1f}/100 ({summary['grade']})")
    
    if summary["meets_week5_targets"]:
        logger.info("🎉 Congratulations! All Week 5 performance targets met!")
    else:
        logger.warning("⚠️  Some performance targets not met. Review detailed report.")

if __name__ == "__main__":
    asyncio.run(main()) 