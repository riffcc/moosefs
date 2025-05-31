#!/usr/bin/env python3
"""
MooseNG Performance Analysis Tool

This script analyzes benchmark results and generates comprehensive performance reports
with insights, recommendations, and comparative analysis across regions and test scenarios.
"""

import json
import csv
import argparse
import sqlite3
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Any, Optional, Tuple
import warnings
warnings.filterwarnings('ignore')

class PerformanceAnalyzer:
    """Main class for analyzing MooseNG performance metrics."""
    
    def __init__(self, data_dir: str, output_dir: str):
        self.data_dir = Path(data_dir)
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Configure plotting style
        plt.style.use('seaborn-v0_8')
        sns.set_palette("husl")
        
        # Initialize data storage
        self.metrics_data = {}
        self.benchmark_results = {}
        self.analysis_results = {}
        
    def load_data(self) -> None:
        """Load all benchmark data from files."""
        print(f"Loading data from {self.data_dir}...")
        
        # Load JSON benchmark results
        for json_file in self.data_dir.glob("*-results.json"):
            category = json_file.stem.replace("-results", "")
            try:
                with open(json_file, 'r') as f:
                    self.benchmark_results[category] = json.load(f)
                print(f"  Loaded {category} benchmark results")
            except Exception as e:
                print(f"  Error loading {json_file}: {e}")
        
        # Load CSV metrics data
        for csv_file in self.data_dir.glob("*.csv"):
            try:
                df = pd.read_csv(csv_file)
                self.metrics_data[csv_file.stem] = df
                print(f"  Loaded {csv_file.stem} metrics data ({len(df)} rows)")
            except Exception as e:
                print(f"  Error loading {csv_file}: {e}")
        
        print(f"Data loading completed. Found {len(self.benchmark_results)} benchmark categories")
    
    def analyze_latency_patterns(self) -> Dict[str, Any]:
        """Analyze latency patterns across regions and operations."""
        print("Analyzing latency patterns...")
        
        latency_analysis = {
            'regional_comparison': {},
            'operation_breakdown': {},
            'percentile_analysis': {},
            'outlier_detection': {}
        }
        
        for category, results in self.benchmark_results.items():
            if not isinstance(results, list):
                continue
                
            # Group by region and operation
            regional_latencies = {}
            operation_latencies = {}
            
            for result in results:
                if 'operation' not in result or 'mean_time' not in result:
                    continue
                    
                operation = result['operation']
                mean_time_ns = result['mean_time'].get('nanos', 0) + result['mean_time'].get('secs', 0) * 1e9
                mean_time_ms = mean_time_ns / 1e6
                
                # Extract region from operation name if available
                region = 'unknown'
                if '_us_east_' in operation or '_us-east_' in operation:
                    region = 'us-east'
                elif '_eu_west_' in operation or '_eu-west_' in operation:
                    region = 'eu-west'
                elif '_ap_south_' in operation or '_ap-south_' in operation:
                    region = 'ap-south'
                
                # Regional analysis
                if region not in regional_latencies:
                    regional_latencies[region] = []
                regional_latencies[region].append(mean_time_ms)
                
                # Operation analysis
                op_type = operation.split('_')[0] if '_' in operation else operation
                if op_type not in operation_latencies:
                    operation_latencies[op_type] = []
                operation_latencies[op_type].append(mean_time_ms)
            
            # Calculate statistics
            for region, latencies in regional_latencies.items():
                if latencies:
                    latency_analysis['regional_comparison'][f"{category}_{region}"] = {
                        'mean': np.mean(latencies),
                        'median': np.median(latencies),
                        'p95': np.percentile(latencies, 95),
                        'p99': np.percentile(latencies, 99),
                        'std': np.std(latencies),
                        'min': np.min(latencies),
                        'max': np.max(latencies),
                        'count': len(latencies)
                    }
            
            for op_type, latencies in operation_latencies.items():
                if latencies:
                    latency_analysis['operation_breakdown'][f"{category}_{op_type}"] = {
                        'mean': np.mean(latencies),
                        'median': np.median(latencies),
                        'p95': np.percentile(latencies, 95),
                        'p99': np.percentile(latencies, 99),
                        'std': np.std(latencies),
                        'count': len(latencies)
                    }
        
        self.analysis_results['latency'] = latency_analysis
        return latency_analysis
    
    def analyze_throughput_trends(self) -> Dict[str, Any]:
        """Analyze throughput patterns and trends."""
        print("Analyzing throughput trends...")
        
        throughput_analysis = {
            'regional_throughput': {},
            'operation_throughput': {},
            'scalability_analysis': {},
            'bottleneck_identification': {}
        }
        
        for category, results in self.benchmark_results.items():
            if not isinstance(results, list):
                continue
            
            regional_throughput = {}
            operation_throughput = {}
            
            for result in results:
                if 'throughput' not in result or result['throughput'] is None:
                    continue
                
                operation = result['operation']
                throughput = result['throughput']
                
                # Extract region
                region = 'unknown'
                if '_us_east_' in operation or '_us-east_' in operation:
                    region = 'us-east'
                elif '_eu_west_' in operation or '_eu-west_' in operation:
                    region = 'eu-west'
                elif '_ap_south_' in operation or '_ap-south_' in operation:
                    region = 'ap-south'
                
                # Regional throughput
                if region not in regional_throughput:
                    regional_throughput[region] = []
                regional_throughput[region].append(throughput)
                
                # Operation throughput
                op_type = operation.split('_')[0] if '_' in operation else operation
                if op_type not in operation_throughput:
                    operation_throughput[op_type] = []
                operation_throughput[op_type].append(throughput)
            
            # Calculate throughput statistics
            for region, throughputs in regional_throughput.items():
                if throughputs:
                    throughput_analysis['regional_throughput'][f"{category}_{region}"] = {
                        'mean': np.mean(throughputs),
                        'median': np.median(throughputs),
                        'max': np.max(throughputs),
                        'min': np.min(throughputs),
                        'std': np.std(throughputs),
                        'total': np.sum(throughputs),
                        'count': len(throughputs)
                    }
            
            for op_type, throughputs in operation_throughput.items():
                if throughputs:
                    throughput_analysis['operation_throughput'][f"{category}_{op_type}"] = {
                        'mean': np.mean(throughputs),
                        'median': np.median(throughputs),
                        'max': np.max(throughputs),
                        'total': np.sum(throughputs),
                        'count': len(throughputs)
                    }
        
        self.analysis_results['throughput'] = throughput_analysis
        return throughput_analysis
    
    def analyze_network_performance(self) -> Dict[str, Any]:
        """Analyze network-specific performance metrics."""
        print("Analyzing network performance...")
        
        network_analysis = {
            'cross_region_latency': {},
            'network_conditions_impact': {},
            'bandwidth_utilization': {},
            'packet_loss_correlation': {}
        }
        
        # Analyze network simulation results
        if 'network_simulation' in self.benchmark_results:
            results = self.benchmark_results['network_simulation']
            
            network_conditions = {}
            for result in results:
                operation = result['operation']
                
                # Extract network condition
                condition = 'unknown'
                if 'high_quality' in operation:
                    condition = 'high_quality'
                elif 'cross_continent' in operation:
                    condition = 'cross_continent'
                elif 'poor_mobile' in operation:
                    condition = 'poor_mobile'
                elif 'satellite' in operation:
                    condition = 'satellite'
                
                if condition not in network_conditions:
                    network_conditions[condition] = {
                        'latencies': [],
                        'throughputs': [],
                        'error_rates': []
                    }
                
                # Calculate latency in ms
                mean_time_ns = result['mean_time'].get('nanos', 0) + result['mean_time'].get('secs', 0) * 1e9
                mean_time_ms = mean_time_ns / 1e6
                network_conditions[condition]['latencies'].append(mean_time_ms)
                
                if result.get('throughput'):
                    network_conditions[condition]['throughputs'].append(result['throughput'])
            
            # Calculate network impact statistics
            for condition, data in network_conditions.items():
                if data['latencies']:
                    network_analysis['network_conditions_impact'][condition] = {
                        'avg_latency': np.mean(data['latencies']),
                        'p95_latency': np.percentile(data['latencies'], 95),
                        'avg_throughput': np.mean(data['throughputs']) if data['throughputs'] else 0,
                        'throughput_std': np.std(data['throughputs']) if data['throughputs'] else 0,
                        'latency_variability': np.std(data['latencies']) / np.mean(data['latencies']) if data['latencies'] else 0
                    }
        
        self.analysis_results['network'] = network_analysis
        return network_analysis
    
    def generate_insights(self) -> List[str]:
        """Generate actionable insights from the analysis."""
        print("Generating performance insights...")
        
        insights = []
        
        # Latency insights
        if 'latency' in self.analysis_results:
            latency_data = self.analysis_results['latency']['regional_comparison']
            
            # Find highest and lowest latency regions
            if latency_data:
                avg_latencies = {k: v['mean'] for k, v in latency_data.items()}
                best_region = min(avg_latencies, key=avg_latencies.get)
                worst_region = max(avg_latencies, key=avg_latencies.get)
                
                insights.append(f"🎯 Best performing region: {best_region} (avg: {avg_latencies[best_region]:.2f}ms)")
                insights.append(f"⚠️  Highest latency region: {worst_region} (avg: {avg_latencies[worst_region]:.2f}ms)")
                
                # Check for high variability
                for region, stats in latency_data.items():
                    if stats['std'] / stats['mean'] > 0.5:  # High coefficient of variation
                        insights.append(f"📊 High latency variability detected in {region} (CV: {stats['std']/stats['mean']:.2f})")
        
        # Throughput insights
        if 'throughput' in self.analysis_results:
            throughput_data = self.analysis_results['throughput']['regional_throughput']
            
            if throughput_data:
                avg_throughputs = {k: v['mean'] for k, v in throughput_data.items()}
                best_throughput = max(avg_throughputs, key=avg_throughputs.get)
                worst_throughput = min(avg_throughputs, key=avg_throughputs.get)
                
                insights.append(f"🚀 Highest throughput: {best_throughput} ({avg_throughputs[best_throughput]:.2f} ops/s)")
                insights.append(f"🐌 Lowest throughput: {worst_throughput} ({avg_throughputs[worst_throughput]:.2f} ops/s)")
        
        # Network insights
        if 'network' in self.analysis_results:
            network_data = self.analysis_results['network']['network_conditions_impact']
            
            for condition, stats in network_data.items():
                if stats['latency_variability'] > 0.3:
                    insights.append(f"🌐 High network variability under {condition} conditions (CV: {stats['latency_variability']:.2f})")
                
                if stats['avg_latency'] > 500:  # High latency threshold
                    insights.append(f"⏱️  Very high latency detected under {condition} conditions ({stats['avg_latency']:.2f}ms)")
        
        # Performance recommendations
        insights.extend([
            "",
            "📋 Recommendations:",
            "• Consider caching strategies for high-latency regions",
            "• Implement adaptive timeouts based on network conditions", 
            "• Optimize data placement for cross-region access patterns",
            "• Monitor and alert on latency percentiles, not just averages",
            "• Implement circuit breakers for unreliable network conditions"
        ])
        
        return insights
    
    def create_visualizations(self) -> None:
        """Create comprehensive visualizations of the performance data."""
        print("Creating performance visualizations...")
        
        # Set up the plotting style
        plt.rcParams['figure.figsize'] = (12, 8)
        plt.rcParams['font.size'] = 10
        
        # 1. Regional Latency Comparison
        if 'latency' in self.analysis_results:
            self._plot_regional_latency()
        
        # 2. Throughput Analysis
        if 'throughput' in self.analysis_results:
            self._plot_throughput_analysis()
        
        # 3. Network Conditions Impact
        if 'network' in self.analysis_results:
            self._plot_network_impact()
        
        # 4. Performance Heatmap
        self._plot_performance_heatmap()
        
        print(f"Visualizations saved to {self.output_dir}")
    
    def _plot_regional_latency(self) -> None:
        """Plot regional latency comparison."""
        latency_data = self.analysis_results['latency']['regional_comparison']
        
        if not latency_data:
            return
        
        regions = []
        means = []
        p95s = []
        p99s = []
        
        for region_key, stats in latency_data.items():
            regions.append(region_key)
            means.append(stats['mean'])
            p95s.append(stats['p95'])
            p99s.append(stats['p99'])
        
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(15, 6))
        
        # Mean latency comparison
        ax1.bar(regions, means, alpha=0.7, color='skyblue')
        ax1.set_title('Mean Latency by Region')
        ax1.set_ylabel('Latency (ms)')
        ax1.tick_params(axis='x', rotation=45)
        
        # Percentile comparison
        x_pos = np.arange(len(regions))
        width = 0.35
        
        ax2.bar(x_pos - width/2, p95s, width, label='P95', alpha=0.7, color='orange')
        ax2.bar(x_pos + width/2, p99s, width, label='P99', alpha=0.7, color='red')
        ax2.set_title('Latency Percentiles by Region')
        ax2.set_ylabel('Latency (ms)')
        ax2.set_xticks(x_pos)
        ax2.set_xticklabels(regions, rotation=45)
        ax2.legend()
        
        plt.tight_layout()
        plt.savefig(self.output_dir / 'regional_latency_comparison.png', dpi=300, bbox_inches='tight')
        plt.close()
    
    def _plot_throughput_analysis(self) -> None:
        """Plot throughput analysis."""
        throughput_data = self.analysis_results['throughput']['regional_throughput']
        
        if not throughput_data:
            return
        
        regions = list(throughput_data.keys())
        means = [stats['mean'] for stats in throughput_data.values()]
        medians = [stats['median'] for stats in throughput_data.values()]
        maxes = [stats['max'] for stats in throughput_data.values()]
        
        fig, ax = plt.subplots(figsize=(12, 6))
        
        x_pos = np.arange(len(regions))
        width = 0.25
        
        ax.bar(x_pos - width, means, width, label='Mean', alpha=0.7, color='lightblue')
        ax.bar(x_pos, medians, width, label='Median', alpha=0.7, color='lightgreen')
        ax.bar(x_pos + width, maxes, width, label='Max', alpha=0.7, color='lightcoral')
        
        ax.set_title('Throughput Analysis by Region')
        ax.set_ylabel('Throughput (ops/s)')
        ax.set_xlabel('Region')
        ax.set_xticks(x_pos)
        ax.set_xticklabels(regions, rotation=45)
        ax.legend()
        
        plt.tight_layout()
        plt.savefig(self.output_dir / 'throughput_analysis.png', dpi=300, bbox_inches='tight')
        plt.close()
    
    def _plot_network_impact(self) -> None:
        """Plot network conditions impact."""
        if 'network_conditions_impact' not in self.analysis_results.get('network', {}):
            return
        
        network_data = self.analysis_results['network']['network_conditions_impact']
        
        conditions = list(network_data.keys())
        latencies = [stats['avg_latency'] for stats in network_data.values()]
        throughputs = [stats['avg_throughput'] for stats in network_data.values()]
        
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(15, 6))
        
        # Latency by network condition
        bars1 = ax1.bar(conditions, latencies, alpha=0.7, color='coral')
        ax1.set_title('Average Latency by Network Condition')
        ax1.set_ylabel('Latency (ms)')
        ax1.tick_params(axis='x', rotation=45)
        
        # Add value labels on bars
        for bar, latency in zip(bars1, latencies):
            ax1.text(bar.get_x() + bar.get_width()/2, bar.get_height() + max(latencies)*0.01,
                    f'{latency:.1f}ms', ha='center', va='bottom')
        
        # Throughput by network condition
        bars2 = ax2.bar(conditions, throughputs, alpha=0.7, color='lightgreen')
        ax2.set_title('Average Throughput by Network Condition')
        ax2.set_ylabel('Throughput (ops/s)')
        ax2.tick_params(axis='x', rotation=45)
        
        # Add value labels on bars
        for bar, throughput in zip(bars2, throughputs):
            if throughput > 0:
                ax2.text(bar.get_x() + bar.get_width()/2, bar.get_height() + max(throughputs)*0.01,
                        f'{throughput:.1f}', ha='center', va='bottom')
        
        plt.tight_layout()
        plt.savefig(self.output_dir / 'network_conditions_impact.png', dpi=300, bbox_inches='tight')
        plt.close()
    
    def _plot_performance_heatmap(self) -> None:
        """Create a performance heatmap."""
        # Combine latency and throughput data for heatmap
        heatmap_data = []
        
        if 'latency' in self.analysis_results:
            for region_key, stats in self.analysis_results['latency']['regional_comparison'].items():
                heatmap_data.append([
                    region_key, 'Latency (ms)', stats['mean'], 'lower_better'
                ])
        
        if 'throughput' in self.analysis_results:
            for region_key, stats in self.analysis_results['throughput']['regional_throughput'].items():
                heatmap_data.append([
                    region_key, 'Throughput (ops/s)', stats['mean'], 'higher_better'
                ])
        
        if not heatmap_data:
            return
        
        # Create DataFrame for heatmap
        df = pd.DataFrame(heatmap_data, columns=['Region', 'Metric', 'Value', 'Direction'])
        
        # Pivot for heatmap
        pivot_df = df.pivot(index='Region', columns='Metric', values='Value')
        
        # Normalize values for better visualization
        normalized_df = pivot_df.copy()
        for col in normalized_df.columns:
            if 'Latency' in col:
                # For latency, invert scale (lower is better)
                normalized_df[col] = 1 - (normalized_df[col] - normalized_df[col].min()) / (normalized_df[col].max() - normalized_df[col].min())
            else:
                # For throughput, normal scale (higher is better)
                normalized_df[col] = (normalized_df[col] - normalized_df[col].min()) / (normalized_df[col].max() - normalized_df[col].min())
        
        plt.figure(figsize=(10, 8))
        sns.heatmap(normalized_df, annot=True, cmap='RdYlGn', fmt='.2f', 
                   cbar_kws={'label': 'Normalized Performance Score'})
        plt.title('Performance Heatmap (Green = Better Performance)')
        plt.tight_layout()
        plt.savefig(self.output_dir / 'performance_heatmap.png', dpi=300, bbox_inches='tight')
        plt.close()
    
    def generate_report(self) -> None:
        """Generate comprehensive performance report."""
        print("Generating comprehensive performance report...")
        
        insights = self.generate_insights()
        
        # Create HTML report
        html_content = f"""
        <!DOCTYPE html>
        <html>
        <head>
            <title>MooseNG Performance Analysis Report</title>
            <style>
                body {{ font-family: Arial, sans-serif; margin: 40px; }}
                .header {{ background-color: #f0f0f0; padding: 20px; border-radius: 5px; }}
                .section {{ margin: 20px 0; }}
                .insight {{ background-color: #e8f4f8; padding: 10px; margin: 5px 0; border-left: 4px solid #2196F3; }}
                .metric {{ background-color: #f9f9f9; padding: 15px; margin: 10px 0; border: 1px solid #ddd; }}
                .chart {{ text-align: center; margin: 20px 0; }}
                pre {{ background-color: #f5f5f5; padding: 10px; overflow-x: auto; }}
                table {{ border-collapse: collapse; width: 100%; }}
                th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
                th {{ background-color: #f2f2f2; }}
            </style>
        </head>
        <body>
            <div class="header">
                <h1>MooseNG Multi-Region Performance Analysis</h1>
                <p>Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}</p>
            </div>
            
            <div class="section">
                <h2>Executive Summary</h2>
                <div class="insight">
                    This report analyzes the performance of MooseNG across multiple regions and network conditions.
                    The analysis includes latency patterns, throughput characteristics, and network performance impact.
                </div>
            </div>
            
            <div class="section">
                <h2>Key Insights</h2>
        """
        
        for insight in insights:
            if insight.strip():
                html_content += f'<div class="insight">{insight}</div>\n'
        
        html_content += """
            </div>
            
            <div class="section">
                <h2>Performance Visualizations</h2>
                <div class="chart">
                    <h3>Regional Latency Comparison</h3>
                    <img src="regional_latency_comparison.png" alt="Regional Latency Comparison" style="max-width: 100%;">
                </div>
                <div class="chart">
                    <h3>Throughput Analysis</h3>
                    <img src="throughput_analysis.png" alt="Throughput Analysis" style="max-width: 100%;">
                </div>
                <div class="chart">
                    <h3>Network Conditions Impact</h3>
                    <img src="network_conditions_impact.png" alt="Network Conditions Impact" style="max-width: 100%;">
                </div>
                <div class="chart">
                    <h3>Performance Heatmap</h3>
                    <img src="performance_heatmap.png" alt="Performance Heatmap" style="max-width: 100%;">
                </div>
            </div>
            
            <div class="section">
                <h2>Detailed Analysis Results</h2>
                <pre>
        """
        
        html_content += json.dumps(self.analysis_results, indent=2, default=str)
        
        html_content += """
                </pre>
            </div>
        </body>
        </html>
        """
        
        # Save HTML report
        with open(self.output_dir / 'performance_report.html', 'w') as f:
            f.write(html_content)
        
        # Save JSON report
        with open(self.output_dir / 'analysis_results.json', 'w') as f:
            json.dump(self.analysis_results, f, indent=2, default=str)
        
        # Save insights as text
        with open(self.output_dir / 'insights.txt', 'w') as f:
            f.write("\\n".join(insights))
        
        print(f"Performance report generated: {self.output_dir / 'performance_report.html'}")

def main():
    parser = argparse.ArgumentParser(description='Analyze MooseNG performance benchmark results')
    parser.add_argument('--data-dir', required=True, help='Directory containing benchmark results')
    parser.add_argument('--output-dir', required=True, help='Directory to save analysis results')
    parser.add_argument('--format', choices=['html', 'json', 'text'], default='html', help='Output format')
    parser.add_argument('--visualizations', action='store_true', help='Generate visualizations')
    
    args = parser.parse_args()
    
    # Create analyzer
    analyzer = PerformanceAnalyzer(args.data_dir, args.output_dir)
    
    try:
        # Load and analyze data
        analyzer.load_data()
        
        if not analyzer.benchmark_results:
            print("No benchmark results found. Please check the data directory.")
            return
        
        # Perform analysis
        analyzer.analyze_latency_patterns()
        analyzer.analyze_throughput_trends()
        analyzer.analyze_network_performance()
        
        # Generate visualizations
        if args.visualizations:
            analyzer.create_visualizations()
        
        # Generate report
        analyzer.generate_report()
        
        print("\\nAnalysis completed successfully!")
        print(f"Results saved to: {args.output_dir}")
        
    except Exception as e:
        print(f"Error during analysis: {e}")
        import traceback
        traceback.print_exc()

if __name__ == '__main__':
    main()