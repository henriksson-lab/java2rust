class BoundaryVertex {
    int value;
    BoundaryVertex(int value) { this.value = value; }
}

class BoundaryEdge {
    BoundaryVertex source;
    BoundaryVertex target;
}

class BoundaryWeightedEdge extends BoundaryEdge {
    int weight;
}

class BoundaryAccess<E extends BoundaryEdge> {
    E retained;

    E connect(E edge, BoundaryVertex source, BoundaryVertex target) {
        if (edge.source == null && edge.target == null) {
            edge.source = source;
            edge.target = target;
        } else if (edge.source != source || edge.target != target) {
            throw new IllegalArgumentException();
        }
        retained = edge;
        return edge;
    }

    BoundaryVertex source(E edge) { return edge.source; }
}

class BoundaryBaseBuilder<G> {
    G value;
    BoundaryBaseBuilder(G value) { this.value = value; }
    G build() { return value; }
}

class BoundaryBuilder<G> extends BoundaryBaseBuilder<G> {
    BoundaryBuilder(G value) { super(value); }
}

class GenericReferenceBoundaries {
    static <T> boolean same(T a, T b) { return a == b; }

    static int run() {
        BoundaryVertex a = new BoundaryVertex(7);
        BoundaryVertex b = new BoundaryVertex(7);
        if (same(a, b)) return 1;
        if (!same(a, a)) return 2;
        if (!same(null, null)) return 3;
        if (same(a, null)) return 4;
        BoundaryWeightedEdge edge = new BoundaryWeightedEdge();
        BoundaryAccess<BoundaryWeightedEdge> access = new BoundaryAccess<>();
        BoundaryWeightedEdge returned = access.connect(edge, a, b);
        if (!same(returned, edge) || !same(access.retained, edge)) return 5;
        if (!same(access.source(edge), a)) return 6;
        returned.source.value = 9;
        if (a.value != 9) return 7;
        if (!same(access.connect(edge, a, b), edge)) return 8;
        boolean rejected = false;
        try { access.connect(edge, b, a); }
        catch (IllegalArgumentException ex) { rejected = true; }
        if (!rejected || !same(edge.source, a)) return 9;
        BoundaryBaseBuilder<BoundaryWeightedEdge> builder = new BoundaryBuilder<>(edge);
        BoundaryWeightedEdge built = builder.build();
        if (!same(built, edge)) return 10;
        built.weight = 11;
        if (edge.weight != 11) return 11;
        BoundaryEdge base = edge;
        if (!same(base, base)) return 12;
        Object erasedBase = base;
        Object erasedAlias = erasedBase;
        if (!same(erasedBase, erasedAlias)) return 13;
        return 0;
    }
}
