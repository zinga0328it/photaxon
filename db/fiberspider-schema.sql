--
-- PostgreSQL database dump
--

\restrict fgxF4SXcCj47Zpvi01cemBL6DMDc5KWO1iWqkWkkuedQM9sRJ8yxCI7G1baZhSb

-- Dumped from database version 17.7 (Ubuntu 17.7-0ubuntu0.25.04.1)
-- Dumped by pg_dump version 17.7 (Ubuntu 17.7-0ubuntu0.25.04.1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: prevent_transaction_events_mutation(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.prevent_transaction_events_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'transaction_events is append-only: UPDATE/DELETE are not allowed';
END;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: activation_otp_challenges; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.activation_otp_challenges (
    challenge_id text NOT NULL,
    activation_id text NOT NULL,
    otp_hash text NOT NULL,
    otp_salt text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    consumed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT activation_otp_challenges_attempts_check CHECK (((attempts >= 0) AND (attempts <= 5)))
);


--
-- Name: activation_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.activation_requests (
    activation_id text NOT NULL,
    technician_id text NOT NULL,
    wr text NOT NULL,
    ont_serial text NOT NULL,
    roe text,
    cabinet_id text,
    splitter_id text,
    notes text,
    status text NOT NULL,
    wr_validation text DEFAULT 'NOT_IMPLEMENTED'::text NOT NULL,
    simulation_result jsonb,
    otp_verified_at timestamp with time zone,
    completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: agent_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.agent_events (
    event_id bigint NOT NULL,
    agent_id text NOT NULL,
    correlation_id text,
    event_type text NOT NULL,
    payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: agent_events_event_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

ALTER TABLE public.agent_events ALTER COLUMN event_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.agent_events_event_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: agents; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.agents (
    agent_id text NOT NULL,
    cabinet_id text,
    role text NOT NULL,
    status text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    runtime_provider text,
    runtime_id text,
    mgmt_ip inet,
    last_seen_at timestamp with time zone
);


--
-- Name: anomalies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.anomalies (
    anomaly_id text NOT NULL,
    correlation_id text,
    transaction_id text,
    severity text NOT NULL,
    code text NOT NULL,
    description text,
    status text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    resolved_at timestamp with time zone
);


--
-- Name: attachment_points; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.attachment_points (
    attachment_point_id text NOT NULL,
    pte_id text,
    status text DEFAULT 'ACTIVE'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: cabinets; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cabinets (
    cabinet_id text NOT NULL,
    status text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cabinets_status_check CHECK ((status = ANY (ARRAY['ACTIVE'::text, 'INACTIVE'::text, 'FAULT'::text])))
);


--
-- Name: customers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.customers (
    customer_id text NOT NULL,
    status text DEFAULT 'ACTIVE'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: ftth_lines; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.ftth_lines (
    line_id text NOT NULL,
    cabinet_id text NOT NULL,
    openstack_network_name text NOT NULL,
    status text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ftth_lines_status_check CHECK ((status = ANY (ARRAY['ACTIVE'::text, 'INACTIVE'::text, 'FAULT'::text])))
);


--
-- Name: onts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.onts (
    ont_id text NOT NULL,
    serial_number text NOT NULL,
    customer_id text,
    current_port_id text,
    status text DEFAULT 'ACTIVE'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: ports; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.ports (
    port_id text NOT NULL,
    splitter_id text NOT NULL,
    pte_id text,
    attachment_point_id text,
    "position" integer,
    resource_state text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ports_resource_state_check CHECK ((resource_state = ANY (ARRAY['FREE'::text, 'RESERVED'::text, 'CONNECTED'::text, 'VERIFIED'::text, 'OCCUPIED'::text, 'FAULT'::text])))
);


--
-- Name: ptes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.ptes (
    pte_id text NOT NULL,
    cabinet_id text,
    line_id text,
    status text DEFAULT 'ACTIVE'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: resource_leases; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.resource_leases (
    lease_id text NOT NULL,
    resource_kind text NOT NULL,
    resource_id text NOT NULL,
    transaction_id text,
    lease_owner text NOT NULL,
    lease_token text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    released_at timestamp with time zone
);


--
-- Name: resource_states; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.resource_states (
    resource_id text NOT NULL,
    resource_kind text NOT NULL,
    resource_state text NOT NULL,
    owner_transaction_id text,
    revision bigint DEFAULT 1 NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT resource_states_resource_state_check CHECK ((resource_state = ANY (ARRAY['FREE'::text, 'RESERVED'::text, 'CONNECTED'::text, 'VERIFIED'::text, 'OCCUPIED'::text, 'FAULT'::text])))
);


--
-- Name: schema_migrations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.schema_migrations (
    version text NOT NULL,
    applied_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: splitters; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.splitters (
    splitter_id text NOT NULL,
    cabinet_id text NOT NULL,
    line_id text NOT NULL,
    ratio integer NOT NULL,
    capacity_total integer NOT NULL,
    secondary_splitter_allowed boolean DEFAULT false NOT NULL,
    status text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT splitters_capacity_total_check CHECK ((capacity_total > 0)),
    CONSTRAINT splitters_ratio_check CHECK ((ratio > 0)),
    CONSTRAINT splitters_status_check CHECK ((status = ANY (ARRAY['ACTIVE'::text, 'INACTIVE'::text, 'FAULT'::text])))
);


--
-- Name: technicians; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.technicians (
    technician_id text NOT NULL,
    display_name text,
    status text DEFAULT 'ACTIVE'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: transaction_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transaction_events (
    event_id bigint NOT NULL,
    transaction_id text,
    correlation_id text,
    event_type text,
    previous_state text,
    new_state text,
    payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: transaction_events_event_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

ALTER TABLE public.transaction_events ALTER COLUMN event_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.transaction_events_event_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: transactions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.transactions (
    transaction_id text NOT NULL,
    idempotency_key text NOT NULL,
    correlation_id text NOT NULL,
    technician_id text,
    customer_id text,
    ont_id text,
    cabinet_id text,
    line_id text,
    splitter_id text,
    pte_id text,
    port_id text,
    attachment_point_id text,
    status text NOT NULL,
    validated_by text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT transactions_status_check CHECK ((status = ANY (ARRAY['RESERVED'::text, 'CONNECTED'::text, 'VERIFIED'::text, 'COMMITTED'::text, 'REJECTED'::text, 'ROLLED_BACK'::text])))
);


--
-- Name: activation_otp_challenges activation_otp_challenges_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activation_otp_challenges
    ADD CONSTRAINT activation_otp_challenges_pkey PRIMARY KEY (challenge_id);


--
-- Name: activation_requests activation_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activation_requests
    ADD CONSTRAINT activation_requests_pkey PRIMARY KEY (activation_id);


--
-- Name: agent_events agent_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.agent_events
    ADD CONSTRAINT agent_events_pkey PRIMARY KEY (event_id);


--
-- Name: agents agents_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.agents
    ADD CONSTRAINT agents_pkey PRIMARY KEY (agent_id);


--
-- Name: anomalies anomalies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.anomalies
    ADD CONSTRAINT anomalies_pkey PRIMARY KEY (anomaly_id);


--
-- Name: attachment_points attachment_points_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attachment_points
    ADD CONSTRAINT attachment_points_pkey PRIMARY KEY (attachment_point_id);


--
-- Name: cabinets cabinets_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cabinets
    ADD CONSTRAINT cabinets_pkey PRIMARY KEY (cabinet_id);


--
-- Name: customers customers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (customer_id);


--
-- Name: ftth_lines ftth_lines_openstack_network_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ftth_lines
    ADD CONSTRAINT ftth_lines_openstack_network_name_key UNIQUE (openstack_network_name);


--
-- Name: ftth_lines ftth_lines_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ftth_lines
    ADD CONSTRAINT ftth_lines_pkey PRIMARY KEY (line_id);


--
-- Name: onts onts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.onts
    ADD CONSTRAINT onts_pkey PRIMARY KEY (ont_id);


--
-- Name: onts onts_serial_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.onts
    ADD CONSTRAINT onts_serial_number_key UNIQUE (serial_number);


--
-- Name: ports ports_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ports
    ADD CONSTRAINT ports_pkey PRIMARY KEY (port_id);


--
-- Name: ptes ptes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ptes
    ADD CONSTRAINT ptes_pkey PRIMARY KEY (pte_id);


--
-- Name: resource_leases resource_leases_lease_token_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.resource_leases
    ADD CONSTRAINT resource_leases_lease_token_key UNIQUE (lease_token);


--
-- Name: resource_leases resource_leases_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.resource_leases
    ADD CONSTRAINT resource_leases_pkey PRIMARY KEY (lease_id);


--
-- Name: resource_states resource_states_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.resource_states
    ADD CONSTRAINT resource_states_pkey PRIMARY KEY (resource_id);


--
-- Name: schema_migrations schema_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.schema_migrations
    ADD CONSTRAINT schema_migrations_pkey PRIMARY KEY (version);


--
-- Name: splitters splitters_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.splitters
    ADD CONSTRAINT splitters_pkey PRIMARY KEY (splitter_id);


--
-- Name: technicians technicians_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.technicians
    ADD CONSTRAINT technicians_pkey PRIMARY KEY (technician_id);


--
-- Name: transaction_events transaction_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_events
    ADD CONSTRAINT transaction_events_pkey PRIMARY KEY (event_id);


--
-- Name: transactions transactions_idempotency_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_idempotency_key_key UNIQUE (idempotency_key);


--
-- Name: transactions transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_pkey PRIMARY KEY (transaction_id);


--
-- Name: idx_activation_otp_challenges_activation_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_activation_otp_challenges_activation_id ON public.activation_otp_challenges USING btree (activation_id);


--
-- Name: uq_resource_leases_active_resource; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX uq_resource_leases_active_resource ON public.resource_leases USING btree (resource_kind, resource_id) WHERE (released_at IS NULL);


--
-- Name: transaction_events trg_transaction_events_no_update_delete; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_transaction_events_no_update_delete BEFORE DELETE OR UPDATE ON public.transaction_events FOR EACH ROW EXECUTE FUNCTION public.prevent_transaction_events_mutation();


--
-- Name: activation_otp_challenges activation_otp_challenges_activation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activation_otp_challenges
    ADD CONSTRAINT activation_otp_challenges_activation_id_fkey FOREIGN KEY (activation_id) REFERENCES public.activation_requests(activation_id) ON DELETE CASCADE;


--
-- Name: activation_requests activation_requests_technician_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activation_requests
    ADD CONSTRAINT activation_requests_technician_id_fkey FOREIGN KEY (technician_id) REFERENCES public.technicians(technician_id);


--
-- Name: agent_events agent_events_agent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.agent_events
    ADD CONSTRAINT agent_events_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES public.agents(agent_id);


--
-- Name: agents agents_cabinet_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.agents
    ADD CONSTRAINT agents_cabinet_id_fkey FOREIGN KEY (cabinet_id) REFERENCES public.cabinets(cabinet_id);


--
-- Name: anomalies anomalies_transaction_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.anomalies
    ADD CONSTRAINT anomalies_transaction_id_fkey FOREIGN KEY (transaction_id) REFERENCES public.transactions(transaction_id);


--
-- Name: attachment_points attachment_points_pte_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attachment_points
    ADD CONSTRAINT attachment_points_pte_id_fkey FOREIGN KEY (pte_id) REFERENCES public.ptes(pte_id);


--
-- Name: ftth_lines ftth_lines_cabinet_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ftth_lines
    ADD CONSTRAINT ftth_lines_cabinet_id_fkey FOREIGN KEY (cabinet_id) REFERENCES public.cabinets(cabinet_id);


--
-- Name: onts onts_current_port_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.onts
    ADD CONSTRAINT onts_current_port_id_fkey FOREIGN KEY (current_port_id) REFERENCES public.ports(port_id);


--
-- Name: onts onts_customer_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.onts
    ADD CONSTRAINT onts_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(customer_id);


--
-- Name: ports ports_attachment_point_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ports
    ADD CONSTRAINT ports_attachment_point_id_fkey FOREIGN KEY (attachment_point_id) REFERENCES public.attachment_points(attachment_point_id);


--
-- Name: ports ports_pte_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ports
    ADD CONSTRAINT ports_pte_id_fkey FOREIGN KEY (pte_id) REFERENCES public.ptes(pte_id);


--
-- Name: ports ports_splitter_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ports
    ADD CONSTRAINT ports_splitter_id_fkey FOREIGN KEY (splitter_id) REFERENCES public.splitters(splitter_id);


--
-- Name: ptes ptes_cabinet_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ptes
    ADD CONSTRAINT ptes_cabinet_id_fkey FOREIGN KEY (cabinet_id) REFERENCES public.cabinets(cabinet_id);


--
-- Name: ptes ptes_line_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ptes
    ADD CONSTRAINT ptes_line_id_fkey FOREIGN KEY (line_id) REFERENCES public.ftth_lines(line_id);


--
-- Name: resource_leases resource_leases_transaction_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.resource_leases
    ADD CONSTRAINT resource_leases_transaction_id_fkey FOREIGN KEY (transaction_id) REFERENCES public.transactions(transaction_id);


--
-- Name: resource_states resource_states_owner_transaction_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.resource_states
    ADD CONSTRAINT resource_states_owner_transaction_id_fkey FOREIGN KEY (owner_transaction_id) REFERENCES public.transactions(transaction_id);


--
-- Name: splitters splitters_cabinet_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.splitters
    ADD CONSTRAINT splitters_cabinet_id_fkey FOREIGN KEY (cabinet_id) REFERENCES public.cabinets(cabinet_id);


--
-- Name: splitters splitters_line_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.splitters
    ADD CONSTRAINT splitters_line_id_fkey FOREIGN KEY (line_id) REFERENCES public.ftth_lines(line_id);


--
-- Name: transaction_events transaction_events_transaction_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transaction_events
    ADD CONSTRAINT transaction_events_transaction_id_fkey FOREIGN KEY (transaction_id) REFERENCES public.transactions(transaction_id);


--
-- Name: transactions transactions_attachment_point_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_attachment_point_id_fkey FOREIGN KEY (attachment_point_id) REFERENCES public.attachment_points(attachment_point_id);


--
-- Name: transactions transactions_cabinet_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_cabinet_id_fkey FOREIGN KEY (cabinet_id) REFERENCES public.cabinets(cabinet_id);


--
-- Name: transactions transactions_customer_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(customer_id);


--
-- Name: transactions transactions_line_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_line_id_fkey FOREIGN KEY (line_id) REFERENCES public.ftth_lines(line_id);


--
-- Name: transactions transactions_ont_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_ont_id_fkey FOREIGN KEY (ont_id) REFERENCES public.onts(ont_id);


--
-- Name: transactions transactions_port_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_port_id_fkey FOREIGN KEY (port_id) REFERENCES public.ports(port_id);


--
-- Name: transactions transactions_pte_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_pte_id_fkey FOREIGN KEY (pte_id) REFERENCES public.ptes(pte_id);


--
-- Name: transactions transactions_splitter_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_splitter_id_fkey FOREIGN KEY (splitter_id) REFERENCES public.splitters(splitter_id);


--
-- Name: transactions transactions_technician_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.transactions
    ADD CONSTRAINT transactions_technician_id_fkey FOREIGN KEY (technician_id) REFERENCES public.technicians(technician_id);


--
-- PostgreSQL database dump complete
--

\unrestrict fgxF4SXcCj47Zpvi01cemBL6DMDc5KWO1iWqkWkkuedQM9sRJ8yxCI7G1baZhSb

