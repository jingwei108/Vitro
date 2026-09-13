#include <stdio.h>
#define MAXV 5
#define INF 65535

struct Edge {
    int u, v, w;
};

void SPFA(struct Edge edges[], int e, int n, int v0) {
    int dist[MAXV];
    int inqueue[MAXV] = {0};
    // 队列容量上界：SPFA 普通队列（非循环）最坏每个节点入队 n 次
    //（负环检测在 count[v] > n 时终止，故上界 n*n）——固定 MAXV 会在
    // 特定输入下越界写 queue[MAXV]（E2E_FAILURES spfa_default 根因）
    int queue[MAXV * MAXV];
    int front = 0, rear = 0;
    int count[MAXV] = {0};
    for (int i = 0; i < n; i++) dist[i] = INF;
    dist[v0] = 0;
    queue[rear++] = v0;
    inqueue[v0] = 1;
    count[v0]++;
    while (front < rear) {
        int u = queue[front++];
        inqueue[u] = 0;
        for (int i = 0; i < e; i++) {
            if (edges[i].u == u) {
                int v = edges[i].v, w = edges[i].w;
                if (dist[u] + w < dist[v]) {
                    dist[v] = dist[u] + w;
                    if (!inqueue[v]) {
                        queue[rear++] = v;
                        inqueue[v] = 1;
                        count[v]++;
                        if (count[v] > n) {
                            printf("negative cycle\n");
                            return;
                        }
                    }
                }
            }
        }
    }
    for (int i = 0; i < n; i++) printf("%d ", dist[i]);
    printf("\n");
}

int main() {
    struct Edge edges[] = {
        {0, 1, -1}, {0, 2, 4}, {1, 2, 3}, {1, 3, 2},
        {1, 4, 2}, {3, 2, 5}, {3, 1, 1}, {4, 3, -3}
    };
    SPFA(edges, 8, 5, 0);
    return 0;
}
