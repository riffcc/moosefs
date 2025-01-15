#ifndef MFS_RAFT_H
#define MFS_RAFT_H

#include "mfsmaster.h"

// Raft states
#define RAFT_STATE_FOLLOWER 0
#define RAFT_STATE_CANDIDATE 1
#define RAFT_STATE_LEADER 2

// Raft functions
void raft_init();
int raft_is_leader();
void raft_become_leader(int term);
void raft_become_follower(int term, int leader_id);
void raft_request_vote(int candidate_id, int term);
void raft_receive_vote(int voter_id, int term);
void raft_receive_heartbeat(int leader_id, int term, double lease_duration);
void raft_check_timeout();

// Helper functions
int get_my_id();
int get_quorum_size();
double monotonic_seconds();
void send_vote_response(int candidate_id, int term);
void request_votes();

#endif // MFS_RAFT_H