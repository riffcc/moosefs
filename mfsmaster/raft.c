#include "raft.h"
#include "mfsmaster.h"
#include "chartsdefs.h"
#include "clocks.h"
#include "mfslog.h"

#define RAFT_LEASE_DURATION 2 // Default lease duration in seconds

typedef struct {
    int leader_id;
    double lease_expiry;
    int term;
    int voted_for;
    int state; // 0 = FOLLOWER, 1 = CANDIDATE, 2 = LEADER
    double last_heartbeat;
    int votes_received;
} RaftState;

static RaftState raft_state = {0};

void raft_init() {
    raft_state.leader_id = -1;
    raft_state.lease_expiry = 0;
    raft_state.term = 0;
    raft_state.voted_for = -1;
    raft_state.state = 0;
    raft_state.last_heartbeat = monotonic_seconds();
    raft_state.votes_received = 0;
}

int raft_is_leader() {
    return raft_state.state == 2 && monotonic_seconds() < raft_state.lease_expiry;
}

void raft_become_leader(int term) {
    raft_state.term = term;
    raft_state.state = 2;
    raft_state.leader_id = get_my_id();
    raft_state.lease_expiry = monotonic_seconds() + RAFT_LEASE_DURATION;
    mfs_log(MFSLOG_NOTICE, 0, "Became leader for term %d", term);
}

void raft_become_follower(int term, int leader_id) {
    raft_state.term = term;
    raft_state.state = 0;
    raft_state.leader_id = leader_id;
    raft_state.lease_expiry = 0;
    mfs_log(MFSLOG_NOTICE, 0, "Became follower of leader %d in term %d", leader_id, term);
}

void raft_request_vote(int candidate_id, int term) {
    if (term > raft_state.term && 
        (raft_state.voted_for == -1 || raft_state.voted_for == candidate_id)) {
        raft_state.voted_for = candidate_id;
        raft_state.term = term;
        send_vote_response(candidate_id, term);
    }
}

void raft_receive_vote(int voter_id, int term) {
    if (term == raft_state.term && raft_state.state == 1) {
        raft_state.votes_received++;
        if (raft_state.votes_received > get_quorum_size()/2) {
            raft_become_leader(term);
        }
    }
}

void raft_receive_heartbeat(int leader_id, int term, double lease_duration) {
    if (term >= raft_state.term) {
        raft_become_follower(term, leader_id);
        raft_state.lease_expiry = monotonic_seconds() + lease_duration;
        raft_state.last_heartbeat = monotonic_seconds();
    }
}

void raft_check_timeout() {
    double now = monotonic_seconds();
    if (raft_state.state != 2 && now - raft_state.last_heartbeat > RAFT_LEASE_DURATION) {
        raft_state.state = 1;
        raft_state.term++;
        raft_state.voted_for = get_my_id();
        raft_state.votes_received = 1;
        raft_state.last_heartbeat = now;
        request_votes();
    }
}
static int my_id = 0;
static int quorum_size = 3;

int get_my_id() {
    return my_id;
}

int get_quorum_size() {
    return quorum_size;
}

void send_vote_response(int candidate_id, int term) {
    // Implementation for sending vote response
}

void request_votes() {
    // Implementation for requesting votes
}