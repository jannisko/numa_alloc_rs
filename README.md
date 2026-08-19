Simple NUMA allocator ala numa.h + a bump allocator wrapping it

exposes:

- low level alloc/free functions ala numa.h:
    - alloc_on_node, free_numa
- allocator API compatible allocators:
    - NumaAllocator, NumaBumpAllocator
