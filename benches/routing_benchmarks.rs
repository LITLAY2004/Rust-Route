use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_route::routing_table::RoutingTable;
use std::net::Ipv4Addr;

fn bench_route_insertion(c: &mut Criterion) {
    c.bench_function("route_insertion", |b| {
        b.iter(|| {
            let mut table = RoutingTable::new();
            for i in 0..1000 {
                let network = Ipv4Addr::new(192, 168, (i / 256) as u8, 0);
                let mask = Ipv4Addr::new(255, 255, 255, 0);
                let next_hop = Ipv4Addr::new(192, 168, 1, 1);
                table.add_static_route(network, mask, next_hop, 1, format!("eth{}", i % 4));
            }
            black_box(table.route_count());
        })
    });
}

fn bench_route_lookup(c: &mut Criterion) {
    let mut table = RoutingTable::new();

    for i in 0..1000 {
        let network = Ipv4Addr::new(192, 168, (i / 256) as u8, 0);
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        let next_hop = Ipv4Addr::new(192, 168, 1, (i % 250 + 2) as u8);
        table.add_static_route(network, mask, next_hop, 1, "eth0".to_string());
    }

    c.bench_function("route_lookup", |b| {
        b.iter(|| {
            let target = Ipv4Addr::new(192, 168, 1, 100);
            black_box(table.find_best_route(&target));
        })
    });
}

fn bench_routing_table_convergence(c: &mut Criterion) {
    c.bench_function("routing_table_convergence", |b| {
        b.iter(|| {
            let mut table = RoutingTable::new();

            for i in 0..100 {
                let network = Ipv4Addr::new(10, (i / 256) as u8, (i % 256) as u8, 0);
                let mask = Ipv4Addr::new(255, 255, 255, 0);
                let next_hop = Ipv4Addr::new(10, 0, 0, 1);
                table.add_static_route(
                    network,
                    mask,
                    next_hop,
                    (i % 15 + 1) as u32,
                    "eth0".to_string(),
                );
            }

            for i in 0..100 {
                let network = Ipv4Addr::new(10, (i / 256) as u8, (i % 256) as u8, 0);
                let mask = Ipv4Addr::new(255, 255, 255, 0);
                let next_hop = Ipv4Addr::new(10, 0, 1, 1);
                table.add_static_route(
                    network,
                    mask,
                    next_hop,
                    (i % 15 + 2) as u32,
                    "eth1".to_string(),
                );
            }

            black_box(table.route_count());
        })
    });
}

fn bench_large_routing_table(c: &mut Criterion) {
    c.bench_function("large_routing_table_operations", |b| {
        b.iter(|| {
            let mut table = RoutingTable::new();

            for i in 0..10_000 {
                let network = Ipv4Addr::new((i / 256) as u8, (i % 256) as u8, 0, 0);
                let mask = Ipv4Addr::new(255, 255, 0, 0);
                let next_hop = Ipv4Addr::new(172, 16, (i % 256) as u8, 1);
                table.add_static_route(network, mask, next_hop, 5, "eth0".to_string());
            }

            for i in 0..1000 {
                let target = Ipv4Addr::new((i / 256) as u8, (i % 256) as u8, 1, 1);
                black_box(table.find_best_route(&target));
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
