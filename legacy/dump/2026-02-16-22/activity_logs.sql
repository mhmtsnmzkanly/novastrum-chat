--
-- PostgreSQL database dump
--

\restrict JiOrPESce9o8tWp4m0Q8YPQ1H2vJBtedlUSYW6G31KUkGVphaiOsb0iGFDO7LoW

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
-- Name: activity_logs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.activity_logs (
    id uuid NOT NULL,
    action text NOT NULL,
    user_id uuid,
    details jsonb NOT NULL,
    partition_month text NOT NULL,
    created_at timestamp with time zone NOT NULL
);


ALTER TABLE public.activity_logs OWNER TO postgres;

--
-- Data for Name: activity_logs; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.activity_logs (id, action, user_id, details, partition_month, created_at) FROM stdin;
\.


--
-- Name: activity_logs activity_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.activity_logs
    ADD CONSTRAINT activity_logs_pkey PRIMARY KEY (id);


--
-- Name: idx_activity_logs_partition; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_activity_logs_partition ON public.activity_logs USING btree (partition_month, created_at DESC);


--
-- Name: activity_logs activity_logs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.activity_logs
    ADD CONSTRAINT activity_logs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- PostgreSQL database dump complete
--

\unrestrict JiOrPESce9o8tWp4m0Q8YPQ1H2vJBtedlUSYW6G31KUkGVphaiOsb0iGFDO7LoW

