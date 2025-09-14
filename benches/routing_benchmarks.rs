use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_route::{Route, RoutingTable};
use std::net::Ipv4Addr;

fn bench_route_insertion(c: &mut Criterion) {
    c.bench_function("route_insertion", |b| {
        b.iter(|| {
            let mut table = RoutingTable::new();
            for i in 0..1000 {
                let network = Ipv4Addr::new(192, 168, (i / 256) as u8, (i % 256) as u8);
                let route = Route::new(
                    network.to_string().as_str(),
                    24,
                    "192.168.1.1",
                    1
                );
                table.add_route(black_box(route));
            }
        })
    });
}

fn bench_route_lookup(c: &mut Criterion) {
    let mut table = RoutingTable::new();
    
    // Pre-populate routing table
    for i in 0..1000 {
        let network = Ipv4Addr::new(192, 168, (i / 256) as u8, (i % 256) as u8);
        let route = Route::new(
            network.to_string().as_str(),
            24,
            "192.168.1.1",
            1
        );
        table.add_route(route);
    }

    c.bench_function("route_lookup", |b| {
        b.iter(|| {
            let target = Ipv4Addr::new(192, 168, 1, 100);
            table.find_route(black_box(&target.to_string()))
        })
    });
}

fn bench_routing_table_convergence(c: &mut Criterion) {
    c.bench_function("routing_table_convergence", |b| {
        b.iter(|| {
            let mut table = RoutingTable::new();
            
            // Simulate route updates
            for i in 0..100 {
                let network = Ipv4Addr::new(10, 0, (i / 256) as u8, (i % 256) as u8);
                let route = Route::new(
                    network.to_string().as_str(),
                    24,
                    "10.0.1.1",
                    (i % 16) + 1
                );
                table.add_route(black_box(route));
            }
            
            // Update existing routes with new metrics
            for i in 0..100 {
                let network = Ipv4Addr::new(10, 0, (i / 256) as u8, (i % 256) as u8);
                let updated_route = Route::new(
                    network.to_string().as_str(),
                    24,
                    "10.0.1.2",
                    (i % 16) + 2
                );
                table.add_route(black_box(updated_route));
            }
        })
    });
}

fn bench_large_routing_table(c: &mut Criterion) {
    c.bench_function("large_routing_table_operations", |b| {
        b.iter(|| {
            let mut table = RoutingTable::new();
            
            // Insert 10,000 routes
            for i in 0..10000 {
                let a = (i / 16777216) % 256;
                let b = (i / 65536) % 256;
                let c = (i / 256) % 256;
                let d = i % 256;
                
                let network = Ipv4Addr::new(a as u8, b as u8, c as u8, d as u8);
                let route = Route::new(
                    network.to_string().as_str(),
                    24,
                    "192.168.1.1",
                    (i % 15) + 1
                );
                table.add_route(black_box(route));
            }
            
            // Perform lookups
            for i in 0..1000 {
                let target = Ipv4Addr::new(
                    (i / 16777216) % 256,
                    (i / 65536) % 256,
                    (i / 256) % 256,
                    i % 256
                );
                table.find_route(black_box(&target.to_string()));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_route_insertion,
    bench_route_lookup,
    bench_routing_table_convergence,
    bench_large_routing_table
);
criterion_main!(benches);
