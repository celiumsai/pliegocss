use super::*;

#[test]
fn source_index_stores_fanout_relations_once_instead_of_per_site() {
    const COUNT: usize = 512;
    let components = vec![ReachabilityComponent {
        id: "shared".into(),
        sites: (0..COUNT)
            .map(|index| ReachabilitySite {
                file: format!("src/site-{index}.rs"),
                byte_start: index * 2,
                byte_end: index * 2 + 1,
            })
            .collect(),
    }];
    let routes = (0..COUNT)
        .map(|index| ReachabilityRoute {
            id: format!("route-{index:04}"),
            path: format!("/{index}"),
            components: vec!["shared".into()],
        })
        .collect();
    let document = ReachabilityDocument {
        schema: 1,
        application_coverage: "complete".into(),
        components,
        routes,
        islands: Vec::new(),
    };

    let index = document.source_index();
    assert_eq!(index.sites.len(), COUNT);
    assert_eq!(index.sites.values().map(Vec::len).sum::<usize>(), COUNT);
    assert_eq!(
        index
            .routes_by_component
            .values()
            .map(Vec::len)
            .sum::<usize>(),
        COUNT
    );
    assert!(index.islands_by_component.is_empty());

    let evidence = index.origin_evidence("src/site-0.rs", 0, 1).unwrap();
    assert_eq!(evidence.component_ids(), &["shared"]);
    assert_eq!(evidence.route_ids().len(), COUNT);
    assert!(evidence.is_reachable());
}
