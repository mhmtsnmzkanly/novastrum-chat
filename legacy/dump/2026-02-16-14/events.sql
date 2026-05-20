--
-- PostgreSQL database dump
--

\restrict GDziqNKj32joggE6QSACyj88TYcXTvJj1o3tvRIqD5USUeefBOhYHvPkyqK4U3l

-- Dumped from database version 18.1
-- Dumped by pg_dump version 18.1

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

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: events; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.events (
    id uuid NOT NULL,
    event_type smallint NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL
);


ALTER TABLE public.events OWNER TO postgres;

--
-- Data for Name: events; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.events (id, event_type, payload, created_at) FROM stdin;
\.


--
-- Name: events events_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (id);


--
-- Name: idx_events_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_events_created_at ON public.events USING btree (created_at DESC);


--
-- PostgreSQL database dump complete
--

\unrestrict GDziqNKj32joggE6QSACyj88TYcXTvJj1o3tvRIqD5USUeefBOhYHvPkyqK4U3l

