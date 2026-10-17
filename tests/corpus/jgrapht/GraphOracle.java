import java.util.TreeSet;
import org.jgrapht.Graph;
import org.jgrapht.GraphPath;
import org.jgrapht.alg.shortestpath.BFSShortestPath;
import org.jgrapht.alg.shortestpath.DijkstraShortestPath;
import org.jgrapht.graph.DefaultWeightedEdge;
import org.jgrapht.graph.SimpleGraph;
import org.jgrapht.traverse.BreadthFirstIterator;

/** Behavior probe only: production JGraphT sources remain untouched. */
public class GraphOracle {
    private static void edge(Graph<Integer, DefaultWeightedEdge> graph, int from, int to, double weight) {
        DefaultWeightedEdge edge = graph.addEdge(from, to);
        graph.setEdgeWeight(edge, weight);
    }

    private static void emit(String key, Object value) {
        System.out.println(key + "\t" + value);
    }

    public static void main(String[] args) {
        Graph<Integer, DefaultWeightedEdge> graph =
            new SimpleGraph<>(null, DefaultWeightedEdge::new, true);
        for (int vertex = 1; vertex <= 5; vertex++) {
            graph.addVertex(vertex);
        }
        edge(graph, 1, 2, 2.0);
        edge(graph, 2, 3, 3.0);
        edge(graph, 1, 3, 9.0);
        edge(graph, 3, 4, 1.0);
        emit("vertices", graph.vertexSet().size());
        emit("edges", graph.edgeSet().size());
        emit("duplicate_edge_rejected", graph.addEdge(1, 2) == null);
        emit("degree_3", graph.degreeOf(3));

        // Graph iteration order is not a portable contract. Compare reachable
        // membership, and use uniquely optimal paths for sequence comparisons.
        TreeSet<Integer> reached = new TreeSet<>();
        BreadthFirstIterator<Integer, DefaultWeightedEdge> traversal =
            new BreadthFirstIterator<>(graph, 1);
        while (traversal.hasNext()) {
            reached.add(traversal.next());
        }
        emit("bfs_reached", reached);
        GraphPath<Integer, DefaultWeightedEdge> hops =
            new BFSShortestPath<>(graph).getPath(1, 4);
        emit("bfs_path", hops.getVertexList());
        emit("bfs_hops", hops.getLength());

        DijkstraShortestPath<Integer, DefaultWeightedEdge> shortest = new DijkstraShortestPath<>(graph);
        emit("weighted_path", shortest.getPath(1, 4).getVertexList());
        emit("weighted_distance", shortest.getPathWeight(1, 4));
        emit("unreachable", shortest.getPath(1, 5) == null);
        emit("unreachable_distance", shortest.getPathWeight(1, 5));

        graph.removeEdge(2, 3);
        shortest = new DijkstraShortestPath<>(graph);
        emit("after_edge_removal_path", shortest.getPath(1, 4).getVertexList());
        emit("after_edge_removal_distance", shortest.getPathWeight(1, 4));
        emit("vertex_removed", graph.removeVertex(3));
        emit("remaining_vertices", graph.vertexSet().size());
        emit("remaining_edges", graph.edgeSet().size());
        emit("disconnected_after_mutation", new BFSShortestPath<>(graph).getPath(1, 4) == null);
    }
}
